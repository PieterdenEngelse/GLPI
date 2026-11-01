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
    /// `range` is explicit because v1 defaults to 0-50 and silently truncates
    /// past it - the same class of silent short answer the v2.3 client guards
    /// against with its 206 check.
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
        let body = res.text().await.unwrap_or_default();
        // 206 is normal here: v1 answers a ranged collection with Partial
        // Content even when the range covers everything.
        if !status.is_success() {
            bail!("GLPI legacy {url} returned {status}: {body}");
        }
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
