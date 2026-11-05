//! `glpi-cli [--v1] <path>` - GET a GLPI API path and print the JSON.
//!
//! Exists so the API can be inspected without the backend or the browser in
//! the way. When a dashboard shows nothing, this answers "is it GLPI or is it
//! us" in one command.
//!
//!   glpi-cli Inventory/Agent
//!   glpi-cli Assets/Computer
//!   glpi-cli --v1 SoftwareVersion
//!
//! WHY --v1 EXISTS. v2.3 is the default and the one this app prefers, but
//! several inventory collections have no v2.3 route at all - every component,
//! volume and software install is reachable only under a single asset id, and
//! NetworkPort and IPAddress are not reachable at any path. Deciding whether
//! to reach for v1 means being able to see what v1 actually returns first,
//! and guessing at a shape is exactly what this tool exists to prevent.
//!
//! The range is deliberately generous rather than paged: this prints what one
//! request returns, and `Legacy::get` refuses a partial collection outright,
//! so a collection larger than the range fails loudly and names the number to
//! raise instead of printing a short list as if it were whole.
use anyhow::{Context, Result};
use glpi_inv_backend::{config::Config, glpi};

/// The range `--v1` asks for. Not a cap on anything real - it is the point at
/// which the tool admits it is looking at part of a collection.
const V1_RANGE: &str = "0-99999";

/// Which of GLPI's three APIs to ask.
#[derive(PartialEq, Eq)]
enum Api {
    /// `/apirest.php` - what no HL version models: ProfileRight, Profile_User.
    V1,
    /// The default HL version, where every schema this app reads is verified.
    Hl,
    /// HL v3.0, for routes v2.3 has no equivalent for. Not a superset of
    /// v2.3: 42 routes are removed at 3.0.
    V3,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let first = args.next().context(
        "usage: glpi-cli [--v1|--v3] <api-path>   e.g. glpi-cli Inventory/Agent\n\
         needs GLPI_CLIENT_ID and GLPI_CLIENT_SECRET in the environment",
    )?;

    // Three APIs, because GLPI serves three and they answer different
    // questions. --v1 for what no HL version models at all; --v3 for the
    // routes v2.3 has no equivalent for; neither for everything else.
    let (api, path) = match first.as_str() {
        "--v1" => (
            Api::V1,
            args.next()
                .context("--v1 needs a path, e.g. glpi-cli --v1 SoftwareVersion")?,
        ),
        "--v3" => (
            Api::V3,
            args.next()
                .context("--v3 needs a path, e.g. glpi-cli --v3 Security/LoginSession/My")?,
        ),
        _ => (Api::Hl, first),
    };

    let cfg = Config::from_env()?;

    let value = if api == Api::V1 {
        let legacy = glpi::legacy::Legacy::new(&cfg.glpi_url, cfg.username, cfg.password);
        // A shared reference, so the closure can own one while with_session
        // borrows the client itself. Moving `legacy` in would take the client
        // the call is made on.
        let l = &legacy;
        l.with_session(|session| async move {
            let out = l.get(&session, &path, Some(V1_RANGE)).await;
            (session, out)
        })
        .await?
    } else {
        let client = glpi::Client::new(
            &cfg.glpi_url,
            cfg.client_id,
            cfg.client_secret,
            cfg.username,
            cfg.password,
        );
        match api {
            Api::V3 => client.get_at(glpi::HL_V3, &path).await?,
            _ => client.get(&path).await?,
        }
    };

    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
