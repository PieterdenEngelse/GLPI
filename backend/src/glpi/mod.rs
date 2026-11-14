//! Client for GLPI's High-Level API, v2.3.
//!
//! Version choice is deliberate. GLPI 11.0.12 advertises five: v1 (described
//! by GLPI itself as "closely tied to the GLPI source code ... no promise of
//! stability between versions"), v2.0/v2.1/v2.2 (all three marked
//! `"deprecated": true`), and v2.3. Only v2.3 is both current and stable, so
//! it is the only one this client speaks.
//!
//! WHAT THIS API CANNOT GIVE US - verified by reading
//! src/Glpi/Api/HL/Controller/InventoryController.php in the GLPI release:
//!
//!   * RefusedEquipment is NOT exposed. That is the failure mode worth
//!     surfacing most: the agent exits 0, GLPI rejects the submission, and
//!     the asset silently never appears. Nothing in the agent's own log says
//!     so.
//!   * No field-level change history (glpi_logs) under Inventory, so "what
//!     changed between runs" is not reachable in v2.3. v1 does answer it, one
//!     asset per request, via getItem's with_logs - measured, 42 entries for
//!     this install's one computer. Fleet-wide in a single query is what it
//!     cannot do, which is what the database grant is for.
//!
//! Both live in the database, and since 2026-11-13 the account can read all
//! of it: the grant to pde@localhost is server-wide, so the question is no
//! longer which tables are reachable but which this app chooses to read. It
//! reads one, glpi_refusedequipments. The rest is deliberate restraint rather
//! than enforcement - see CLAUDE.md, which records what the grant stopped
//! protecting.
use anyhow::{bail, Context, Result};
use serde::Deserialize;

pub mod agents;
pub mod computers;
pub mod entities;
pub mod infocom;
pub mod inventory;
pub mod legacy;
pub mod posture;
pub mod refused;
pub mod rights;
pub mod rsql;
pub mod schema_check;
pub mod session;
pub mod sessions;
pub mod system_status;

/// An OAuth2 access token with the instant it stops being valid.
struct Token {
    value: String,
    expires_at: std::time::Instant,
}

pub struct Client {
    http: reqwest::Client,
    base: String,
    client_id: String,
    client_secret: String,
    username: String,
    password: String,
    token: tokio::sync::Mutex<Option<Token>>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

impl Client {
    pub fn new(
        glpi_url: &str,
        client_id: String,
        client_secret: String,
        username: String,
        password: String,
    ) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: glpi_url.trim_end_matches('/').to_string(),
            client_id,
            client_secret,
            username,
            password,
            token: tokio::sync::Mutex::new(None),
        }
    }

    /// A valid bearer token, fetched on first use and reused until it is close
    /// to expiry.
    ///
    /// The 60-second margin is not decoration: without it a token that passes
    /// the check here can still expire in flight, and the request comes back
    /// 401 for a reason that looks nothing like "the token aged out".
    async fn token(&self) -> Result<String> {
        let mut guard = self.token.lock().await;

        if let Some(t) = guard.as_ref() {
            if t.expires_at > std::time::Instant::now() + std::time::Duration::from_secs(60) {
                return Ok(t.value.clone());
            }
        }

        let url = format!("{}/api.php/token", self.base);
        let res = self
            .http
            .post(&url)
            // The `password` grant, NOT `client_credentials`. GLPI issues a
            // perfectly valid token for client_credentials - Bearer, 3600s,
            // an 807-character JWT - and then rejects it on every data
            // endpoint with 401 "The Authorization header is missing or
            // invalid", which reads like a malformed header rather than the
            // wrong grant.
            //
            // Router.php:717 is the reason:
            //
            //   if (!($request->hasHeader('Authorization')
            //         && Session::getLoginUserID() !== false)) { 401 }
            //
            // It requires a logged-in USER, and a client_credentials token has
            // no user attached by definition. So there is no configuration
            // that makes that grant work here; only a user-scoped one can.
            //
            // scope=api because /Inventory/Agent falls into the default branch
            // of OAuthRequestMiddleware, which requires the `api` scope. The
            // OAuth client in GLPI must have it ticked, or this returns
            // "You do not have the required scope(s)".
            .form(&[
                ("grant_type", "password"),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
                ("username", &self.username),
                ("password", &self.password),
                ("scope", "api"),
            ])
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            // GLPI answers with a real OAuth2 error document, so pass it
            // through rather than flattening it to "auth failed".
            bail!("GLPI token endpoint returned {status}: {body}");
        }

        let tr: TokenResponse = res.json().await.context("decoding token response")?;
        let value = tr.access_token.clone();
        *guard = Some(Token {
            value: tr.access_token,
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(tr.expires_in),
        });
        Ok(value)
    }

    /// Drop the cached token so the next call mints a fresh one.
    async fn invalidate_token(&self) {
        *self.token.lock().await = None;
    }

    /// One authorised GET. Separate from `get` only so the 401 path can send
    /// the identical request again without rebuilding it.
    async fn send(&self, url: &str, token: String) -> Result<reqwest::Response> {
        self.http
            .get(url)
            .bearer_auth(token)
            .header("Accept", "application/json")
            .send()
            .await
            .with_context(|| format!("GET {url}"))
    }

    /// One authorised GET, with the 401 retry. Returns the status, the
    /// `Content-Range` header and the body.
    ///
    /// Separate from `get` so every page of a walked collection gets the retry
    /// independently - a token can age out between page three and page four.
    async fn fetch(&self, url: &str) -> Result<(reqwest::StatusCode, Option<String>, String)> {
        let mut res = self.send(url, self.token().await?).await?;

        // One retry on 401, with the cached token thrown away first.
        //
        // The expiry check in token() can only see the JWT's own clock, and
        // that is not the whole story. This API authorises per USER, so the
        // token rides on a server-side session that can end independently -
        // a GLPI restart, a session garbage collection - while the JWT still
        // looks valid for another half hour. The symptom is a 401 that flatly
        // contradicts a token this client believes in, and without a retry the
        // dashboard shows an upstream error until the clock happens to catch
        // up on its own.
        //
        // Exactly once, deliberately. A 401 that survives a token minted
        // seconds ago is a real authorisation problem - missing `api` scope, a
        // revoked client, a disabled service user - and looping on it would
        // turn a clear failure into a slow one while adding load to the thing
        // already refusing us.
        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            tracing::info!("GLPI {url} returned 401; retrying once with a fresh token");
            self.invalidate_token().await;
            res = self.send(url, self.token().await?).await?;
        }

        let status = res.status();
        // Read before the body is consumed. This header is the only place the
        // full count appears; the JSON body is a bare array with no envelope.
        let content_range = res
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = res.text().await.unwrap_or_default();
        Ok((status, content_range, body))
    }

    /// GET a v2.3 path, e.g. "Inventory/Agent", walking it to the end.
    ///
    /// WHY THIS PAGES. v2.3 caps a collection at 100 rows by default and says
    /// so with 206 plus `Content-Range: start-end/total`. This client used to
    /// send no paging parameters at all and refuse any 206, on the stated
    /// grounds that "GLPI returns every row with a 200" - which was simply
    /// false, and invisible only because every collection on the install it
    /// was written against held fewer than 100 rows. Measured on 11.0.12:
    ///
    ///   Assets/Software                  ->  206  Content-Range: 0-99/1330
    ///   Assets/Software?limit=500        ->  206  Content-Range: 0-499/1330
    ///   Assets/Software?start=1300       ->  200  Content-Range: 1300-1329/1330
    ///
    /// So the old behaviour would have turned the Agents and Computers pages
    /// into a 502 on any GLPI holding more than a hundred of either - which is
    /// to say on any real fleet.
    ///
    /// `limit` is honoured rather than clamped, so `PAGE` is a round-trip
    /// tuning choice and nothing more. That is the point of walking: with a
    /// loop, no page size can truncate the answer, where before the size WAS
    /// the answer's limit.
    ///
    /// Non-collection paths are unaffected. `/session` ignores the parameters
    /// and returns no `Content-Range`, which is how this tells the two apart -
    /// verified, not assumed.
    pub async fn get(&self, path: &str) -> Result<serde_json::Value> {
        self.get_at(HL_DEFAULT, path).await
    }

    /// One object, where a 404 means "no such record" rather than an error.
    ///
    /// Most of this API uses 404 for a route that does not exist, which `get`
    /// is right to treat as a failure. A few endpoints use it for a record
    /// that does not exist, and conflating the two turns an ordinary absence
    /// into an upstream error on screen.
    ///
    /// `Assets/{itemtype}/{id}/Infocom` is the case this was written for:
    /// `AssetController::getItemInfocom` ends with
    /// `if (empty($results)) return self::getNotFoundErrorResponse();`, so an
    /// asset with no financial record answers 404 while the route is present
    /// and working. A missing `infocom` right is a separate branch in the same
    /// handler and answers access-denied, so `None` here never hides a
    /// permission problem.
    ///
    /// Single request, no walk: this is for one object, and an endpoint that
    /// returns a collection should go through `get`.
    pub async fn get_optional_at(
        &self,
        version: &str,
        path: &str,
    ) -> Result<Option<serde_json::Value>> {
        let url = format!(
            "{}/api.php/{}/{}",
            self.base,
            version.trim_matches('/'),
            path.trim_start_matches('/')
        );
        let (status, _, body) = self.fetch(&url).await?;

        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            bail!("GLPI {url} returned {status}: {body}");
        }
        Ok(Some(
            serde_json::from_str(&body).with_context(|| format!("decoding JSON from {url}"))?,
        ))
    }

    /// The same walk, against a named HL version.
    ///
    /// Exists because `get` used to build the version into the URL inline, so
    /// v2.3 was the only version this client could reach at all. Everything
    /// GLPI serves outside it needed a second client or an edit to that line.
    ///
    /// The walk itself is version-independent: the pagination block that sets
    /// the status and the `Content-Range` header is byte-identical between
    /// 11.0.12 and 12.0.1 (`ResourceAccessor.php`), so `start`/`limit`, the
    /// header format and 206-versus-200 all mean the same thing on v3 as on
    /// v2.3. That was checked against the source and measured against both
    /// running versions, not assumed.
    ///
    /// Prefer `get`. v2.3 is where every schema this app reads has been
    /// verified, and v3.0 is not a superset of it - it REMOVES 42 routes that
    /// v2.3 still serves. Reach for this only for something v2.3 genuinely
    /// does not have, and check `/api.php` first for what the install
    /// actually serves rather than assuming a version exists.
    pub async fn get_at(&self, version: &str, path: &str) -> Result<serde_json::Value> {
        let base = format!(
            "{}/api.php/{}/{}",
            self.base,
            version.trim_matches('/'),
            path.trim_start_matches('/')
        );
        let sep = if base.contains('?') { '&' } else { '?' };

        let mut collected: Vec<serde_json::Value> = Vec::new();
        let mut start: i64 = 0;

        loop {
            let url = format!("{base}{sep}start={start}&limit={PAGE}");
            let (status, content_range, body) = self.fetch(&url).await?;
            if !status.is_success() {
                bail!("GLPI {url} returned {status}: {body}");
            }
            let value: serde_json::Value =
                serde_json::from_str(&body).with_context(|| format!("decoding JSON from {url}"))?;

            // No Content-Range means this is not a paged collection - an
            // object endpoint like /session. Hand it back untouched.
            let Some((got_start, got_end, total)) =
                content_range.as_deref().and_then(parse_content_range)
            else {
                if start == 0 {
                    return Ok(value);
                }
                bail!(
                    "GLPI {url} dropped its Content-Range midway through a collection, so \
                     there is no way to tell whether the rest arrived. Refusing rather than \
                     returning {} rows of an unknown total.",
                    collected.len()
                );
            };

            let page: Vec<serde_json::Value> = match value {
                serde_json::Value::Array(a) => a,
                // A single-page collection in some envelope other than a bare
                // array: give it back whole and let the caller unwrap it, as
                // it did before paging existed.
                other => {
                    if start == 0 && got_end + 1 >= total {
                        return Ok(other);
                    }
                    bail!(
                        "GLPI {url} answered a multi-page collection with something other \
                         than an array, so the pages cannot be joined"
                    );
                }
            };

            let empty = page.is_empty();
            collected.extend(page);

            // Done when the window reaches the end. `total` is re-read every
            // page rather than remembered, so a collection that shrinks under
            // us finishes early instead of looping for rows that no longer
            // exist.
            if got_end + 1 >= total {
                break;
            }

            // A page that advances nothing would otherwise loop forever. This
            // fires if GLPI reports more rows than it will serve.
            if empty {
                bail!(
                    "GLPI {url} reported rows {got_start}-{got_end} of {total} but returned \
                     none, so walking it cannot progress. Refusing rather than returning \
                     {} rows as the whole collection.",
                    collected.len()
                );
            }

            start = got_end + 1;
        }

        Ok(serde_json::Value::Array(collected))
    }
}

/// The HL version every call goes to unless told otherwise.
///
/// v2.3 and not v3.0 deliberately. v3.0 exists on 12.0.1 and is not
/// deprecated, but it is not a superset: `Router.php` serves 1, 2.3 and 3.0,
/// and 42 routes are REMOVED at 3.0 - the `SoftwareLicense` compatibility
/// shapes and the knowledge-base `/Category` endpoints. Every schema this app
/// reads is verified against v2.3; none of it is verified against v3.0.
pub const HL_DEFAULT: &str = "v2.3";

/// The version carrying routes v2.3 has no equivalent for.
///
/// On GLPI 12.0.1 that is four `LoginSession` routes and four generic
/// `{itemtype}/{id}/Infocom` routes, under the `/Security` and `/Management`
/// prefixes. It is NOT where the three things this app works around live:
/// `Profile_User` and `RefusedEquipment` both answer 404 here, and the
/// `Profile` schema carries no rights, so v1 and the SQL grant survive v3.
/// Measured against a running 12.0.1, not inferred.
///
/// A GLPI that does not serve v3 answers 404, which is loud. Read `/api.php`
/// rather than assuming this constant applies to whatever is deployed.
pub const HL_V3: &str = "v3.0";

/// Rows to ask for per request when walking a collection.
///
/// Not a cap on anything: `get` loops until GLPI says the window reached the
/// end, so this only decides how many round trips that takes. 500 because
/// GLPI honours it - `limit` is not clamped to the default 100 - and because
/// three requests for a 1330-row collection is a reasonable trade against
/// holding a larger response in memory.
const PAGE: i64 = 500;

/// Pull `start`, `end` and `total` out of a `Content-Range: start-end/total`.
///
/// GLPI builds it in ResourceAccessor.php:386-393 of 11.0.12:
///
///   $has_more = $results['start'] + $results['limit'] < $results['total'];
///   return new JSONResponse($results['results'], $has_more ? 206 : 200, [
///       'Content-Range' => $results['start'] . '-' . $end . '/' . $results['total'],
///   ]);
///
/// Returns None for anything that does not parse, which `get` treats as "not a
/// paged collection" on the first page and as a hard error later - a header
/// that stops being readable midway is not something to guess about.
fn parse_content_range(h: &str) -> Option<(i64, i64, i64)> {
    let (range, total) = h.trim().split_once('/')?;
    let (start, end) = range.split_once('-')?;
    Some((
        start.trim().parse().ok()?,
        end.trim().parse().ok()?,
        total.trim().parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::parse_content_range;

    /// The shapes GLPI 11.0.12 actually sends, measured.
    #[test]
    fn parses_the_header_glpi_sends() {
        assert_eq!(parse_content_range("0-99/1330"), Some((0, 99, 1330)));
        assert_eq!(parse_content_range("0-499/1330"), Some((0, 499, 1330)));
        assert_eq!(
            parse_content_range("1300-1329/1330"),
            Some((1300, 1329, 1330))
        );
        // A single row, and an empty collection.
        assert_eq!(parse_content_range("0-0/1"), Some((0, 0, 1)));
        assert_eq!(parse_content_range("0-0/0"), Some((0, 0, 0)));
    }

    /// Whitespace from a proxy must not defeat it.
    #[test]
    fn tolerates_surrounding_whitespace() {
        assert_eq!(parse_content_range(" 0-99/1330 "), Some((0, 99, 1330)));
    }

    /// Anything unparseable yields None rather than a guess. `get` reads that
    /// as "not a paged collection" on the first page and refuses outright
    /// later, which is the whole point: a header that stops being readable
    /// midway through a walk is not something to assume about.
    #[test]
    fn refuses_to_guess_at_a_header_it_cannot_read() {
        for h in [
            "",
            "items 0-99/1330",
            "0-99",
            "1330",
            "0/1330",
            "a-b/c",
            "0-99/",
            "-/-",
        ] {
            assert_eq!(parse_content_range(h), None, "should not parse: {h:?}");
        }
    }

    /// The arithmetic `get` loops on, stated as a test so the end condition is
    /// pinned rather than inferred from the loop body.
    ///
    /// GLPI's own rule is `start + limit < total` means more remain; this
    /// client works from the end of the served window instead, because that is
    /// what the header reports and it stays correct when GLPI serves fewer
    /// rows than the limit asked for - which is exactly what the last page
    /// does.
    #[test]
    fn the_walk_ends_when_the_window_reaches_the_total() {
        let more = |h: &str| {
            let (_, end, total) = parse_content_range(h).unwrap();
            end + 1 < total
        };
        assert!(more("0-99/1330"), "100 of 1330 is not the end");
        assert!(more("0-499/1330"));
        assert!(more("500-999/1330"));
        assert!(!more("1000-1329/1330"), "the window reached the total");
        assert!(!more("0-0/1"), "a single row is already complete");
        assert!(!more("0-0/0"), "an empty collection is complete");
    }
}
