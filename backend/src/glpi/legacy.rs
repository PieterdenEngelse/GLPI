//! Client for GLPI's legacy v1 API, used for the few things v2.3 does not
//! expose at all.
//!
//! WHY THIS EXISTS, given that glpi/mod.rs argues for v2.3 and quotes GLPI's
//! own warning that v1 is "closely tied to the GLPI source code ... no
//! promise of stability between versions":
//!
//! Verified by reading src/Glpi/Api/HL/ in GLPI 11.0.12 - `Profile_User` has
//! ZERO occurrences anywhere under it, and the v2.3 `Profile` schema carries
//! only id, name, comment, is_default, helpdesk_hardware, helpdesk_item_type,
//! managed_domainrecordtypes and mfa_enforced. No `interface`, no rights. So
//! v2.3 can report the rights of the session it is already in, and nothing
//! about any other profile or any other authorization.
//!
//! That gap is exactly what makes a rights problem hard to diagnose: the
//! dashboard can say "something is missing" but not "this profile, on this
//! entity, is the one to fix". v1 answers it, so v1 is used for that and
//! nothing else.
//!
//! This does NOT bypass GLPI's permissions the way reading MariaDB directly
//! would - every call is still made as the service account and still filtered
//! by what its profile allows.
//!
//! Authentication is separate and unavoidable: v1 rejects the OAuth bearer
//! token outright with ERROR_SESSION_TOKEN_MISSING, and its initSession
//! rejects a bearer with ERROR_LOGIN_PARAMETERS_MISSING. It wants Basic auth
//! exchanged for a session token.
use anyhow::{bail, Context, Result};

pub struct Legacy {
    http: reqwest::Client,
    base: String,
    username: String,
    password: String,
}

/// A session token that kills itself when dropped is not possible here - Drop
/// cannot await - so callers use `with_session`, which kills it on every exit
/// path. GLPI sessions otherwise accumulate server-side until they time out.
pub struct Session {
    token: String,
}

impl Legacy {
    pub fn new(glpi_url: &str, username: String, password: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base: glpi_url.trim_end_matches('/').to_string(),
            username,
            password,
        }
    }

    async fn init_session(&self) -> Result<Session> {
        let url = format!("{}/apirest.php/initSession", self.base);
        let res = self
            .http
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;

        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("GLPI legacy initSession returned {status}: {body}");
        }

        let v: serde_json::Value =
            serde_json::from_str(&body).context("decoding initSession response")?;
        let token = v
            .get("session_token")
            .and_then(|t| t.as_str())
            .context("initSession returned no session_token")?
            .to_string();
        Ok(Session { token })
    }

    async fn kill_session(&self, session: &Session) {
        let url = format!("{}/apirest.php/killSession", self.base);
        // Best effort. A leaked session times out on its own, and failing the
        // caller's request because cleanup failed would be the wrong trade.
        let _ = self
            .http
            .get(&url)
            .header("Session-Token", &session.token)
            .send()
            .await;
    }

    /// GET a v1 path within a session, e.g. "Profile_User".
    ///
    /// `range` is explicit because v1 applies a default limit and truncates
    /// past it. Measured against 11.0.12, a rangeless
    /// `GET /apirest.php/ProfileRight` returns 15 of 899 rows - not the 50
    /// this comment used to claim.
    ///
    /// Truncation is REFUSED here rather than passed on, the same way the
    /// v2.3 client refuses a 206. See `reject_partial` below for why v1's
    /// status code is trustworthy enough to decide on.
    pub async fn get(
        &self,
        session: &Session,
        path: &str,
        range: Option<&str>,
    ) -> Result<serde_json::Value> {
        let url = match range {
            Some(r) => format!(
                "{}/apirest.php/{}?range={}",
                self.base,
                path.trim_start_matches('/'),
                r
            ),
            None => format!("{}/apirest.php/{}", self.base, path.trim_start_matches('/')),
        };
        let res = self
            .http
            .get(&url)
            .header("Session-Token", &session.token)
            .header("Accept", "application/json")
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;

        let status = res.status();
        // Read before the body is consumed; this header carries the only count
        // of what was withheld.
        let content_range = res
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = res.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("GLPI legacy {url} returned {status}: {body}");
        }
        reject_partial(status, content_range.as_deref(), &url)?;
        serde_json::from_str(&body).with_context(|| format!("decoding JSON from {url}"))
    }

    /// Run `f` inside a session, killing it afterwards whether or not `f`
    /// succeeded.
    pub async fn with_session<T, F, Fut>(&self, f: F) -> Result<T>
    where
        F: FnOnce(Session) -> Fut,
        Fut: std::future::Future<Output = (Session, Result<T>)>,
    {
        let session = self.init_session().await?;
        let (session, out) = f(session).await;
        self.kill_session(&session).await;
        out
    }
}

/// Refuse a v1 collection that was only partly served.
///
/// This guard was missing, on the strength of a comment claiming "206 is
/// normal here: v1 answers a ranged collection with Partial Content even when
/// the range covers everything". That is not what it does. Measured against
/// 11.0.12, on a 899-row collection:
///
///   range=0-9999  ->  200 OK              Content-Range: 0-898/899
///   range=0-5     ->  206 Partial Content Content-Range: 0-5/899
///   (no range)    ->  206 Partial Content Content-Range: 0-14/899
///
/// So v1 distinguishes complete from partial exactly as v2.3 does, and the
/// status code alone is enough to decide. `reqwest`'s `is_success()` spans
/// 200-299, so without this a 206 passed the check above and a short list
/// rendered as the whole collection - the Authorizations table showing fewer
/// authorizations than exist, with nothing to indicate it.
///
/// The ranges the callers pass are generous (0-999, 0-9999) and nothing here
/// comes close today. That is the point: the limit is an assumption about
/// somebody else's estate, and when it breaks it should break loudly and name
/// the number to raise rather than quietly shorten a list.
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
        "GLPI legacy {url} returned 206 Partial Content ({range}): this is part of the \
         collection, not all of it. Refusing rather than showing a short list as a \
         complete one - raise the `range` this call asks for."
    )
}

#[cfg(test)]
mod tests {
    use super::reject_partial;
    use reqwest::StatusCode;

    /// The operator needs the slice v1 actually served and the true total, or
    /// the error says only that something was incomplete.
    #[test]
    fn a_206_is_refused_and_names_the_range() {
        let err = reject_partial(
            StatusCode::PARTIAL_CONTENT,
            Some("0-14/899"),
            "http://glpi/apirest.php/ProfileRight",
        )
        .expect_err("a partial collection must not pass as complete");
        let msg = format!("{err:#}");
        assert!(msg.contains("0-14/899"), "served slice missing from: {msg}");
        assert!(msg.contains("range"), "no hint at the fix in: {msg}");
    }

    /// A 206 is reason enough on its own; a proxy may strip the header.
    #[test]
    fn a_206_without_the_header_is_still_refused() {
        assert!(reject_partial(StatusCode::PARTIAL_CONTENT, None, "u").is_err());
    }

    /// The case the old comment got wrong: a range covering the whole
    /// collection comes back 200, carrying a Content-Range whose presence must
    /// not be mistaken for truncation.
    #[test]
    fn a_complete_200_carrying_a_content_range_passes() {
        assert!(reject_partial(StatusCode::OK, Some("0-898/899"), "u").is_ok());
    }

    /// Non-collection endpoints send no Content-Range at all.
    #[test]
    fn a_200_without_a_content_range_passes() {
        assert!(reject_partial(StatusCode::OK, None, "u").is_ok());
    }
}
