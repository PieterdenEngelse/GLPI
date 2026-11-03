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
//!     changed between runs" is not reachable either.
//!
//! Both live in the database. Options are a read-only MariaDB user scoped to
//! those two tables, or doing without; that choice is not made here. See
//! CLAUDE.md.
use anyhow::{bail, Context, Result};
use serde::Deserialize;

pub mod agents;
pub mod computers;
pub mod entities;
pub mod legacy;
pub mod posture;
pub mod rights;
pub mod session;

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

    /// GET a v2.3 path, e.g. "Inventory/Agent".
    pub async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let url = format!(
            "{}/api.php/v2.3/{}",
            self.base,
            path.trim_start_matches('/')
        );
        let mut res = self.send(&url, self.token().await?).await?;

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
            res = self.send(&url, self.token().await?).await?;
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
        if !status.is_success() {
            bail!("GLPI {url} returned {status}: {body}");
        }
        reject_partial(status, content_range.as_deref(), &url)?;
        serde_json::from_str(&body).with_context(|| format!("decoding JSON from {url}"))
    }
}

/// Refuse a collection GLPI only partly served.
///
/// GLPI answers a truncated collection with 206 and a `Content-Range` of
/// `start-end/total` - ResourceAccessor.php:386-393 in 11.0.12:
///
///   $has_more = $results['start'] + $results['limit'] < $results['total'];
///   return new JSONResponse($results['results'], $has_more ? 206 : 200, [
///       'Content-Range' => $results['start'] . '-' . $end . '/' . $results['total'],
///   ]);
///
/// reqwest's `is_success()` spans 200-299, so a 206 passes the status check
/// above untouched and the short array renders as the entire fleet. For an app
/// whose only job is noticing which agents stopped reporting, a silently short
/// list is the worst available answer: a missing agent and an unlisted one look
/// identical, and the page reads as a healthy fleet either way.
///
/// This cannot fire today. The client sends no `start` or `limit`, and
/// Search.php:581 adds a SQL LIMIT only when one of those is numeric, so GLPI
/// returns every row with a 200. That is precisely why the guard is here: the
/// invariant is an assumption about an upstream nobody here controls, and when
/// it breaks it does so quietly.
fn reject_partial(
    status: reqwest::StatusCode,
    content_range: Option<&str>,
    url: &str,
) -> Result<()> {
    if status != reqwest::StatusCode::PARTIAL_CONTENT {
        return Ok(());
    }
    let range = content_range.unwrap_or("no Content-Range header");
    bail!(
        "GLPI {url} returned 206 Partial Content ({range}): this is part of the \
         collection, not all of it. This client requests no pagination, so every \
         row should arrive with a 200 - refusing rather than showing a short \
         fleet as the whole fleet. Paginate explicitly if this is now expected."
    )
}

#[cfg(test)]
mod tests {
    use super::reject_partial;
    use reqwest::StatusCode;

    /// The operator needs the slice GLPI actually served, or the error says
    /// only that something was incomplete.
    #[test]
    fn a_206_is_refused_and_names_the_range() {
        let err = reject_partial(
            StatusCode::PARTIAL_CONTENT,
            Some("0-49/327"),
            "http://glpi/api.php/v2.3/Inventory/Agent",
        )
        .expect_err("a partial collection must not pass as complete");
        let msg = format!("{err:#}");
        assert!(msg.contains("0-49/327"), "served slice missing from: {msg}");
    }

    /// A 206 is reason enough on its own; a proxy may strip the header.
    #[test]
    fn a_206_without_the_header_is_still_refused() {
        assert!(reject_partial(StatusCode::PARTIAL_CONTENT, None, "u").is_err());
    }

    /// GLPI sets Content-Range on every collection response, 200 included, so
    /// its presence must not be mistaken for truncation.
    #[test]
    fn a_complete_200_carrying_a_content_range_passes() {
        assert!(reject_partial(StatusCode::OK, Some("0-0/1"), "u").is_ok());
    }
}
