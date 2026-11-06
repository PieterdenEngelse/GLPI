use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use glpi_inv_backend::{api, config::Config, glpi, logs};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "glpi_inv_backend=info,tower_http=warn".into()),
        )
        .init();

    let cfg = Config::from_env()?;
    tracing::info!("GLPI at {}", cfg.glpi_url);

    // Serve the dx bundle if it has been built. dx writes under
    // target/dx/<crate>/<profile>/web/public - overridable because that path
    // is a dx implementation detail, not a promise.
    let static_dir = std::env::var("GLPI_INV_STATIC_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .map(|exe| bundle_candidates(&exe))
                .unwrap_or_default()
                .into_iter()
                .find(|d| d.join("index.html").is_file())
        });

    match &static_dir {
        Some(d) => tracing::info!("serving web bundle from {}", d.display()),
        None => {
            tracing::info!("no web bundle found; API only (build it with `just build-frontend`)")
        }
    }

    let state = Arc::new(api::AppState {
        static_dir,
        glpi_url: cfg.glpi_url.clone(),
        bind: cfg.bind.clone(),
        // A bad URL disables the feature with a warning rather than refusing
        // to start: the other five pages do not need it.
        db: cfg
            .db_url
            .as_deref()
            .and_then(|u| match glpi::refused::Db::new(u) {
                Ok(db) => {
                    tracing::info!("GLPI database configured for refused submissions");
                    Some(db)
                }
                Err(e) => {
                    tracing::warn!("GLPI_DB_URL unusable, Refused page disabled: {e:#}");
                    None
                }
            }),
        legacy: glpi::legacy::Legacy::new(
            &cfg.glpi_url,
            cfg.username.clone(),
            cfg.password.clone(),
        ),
        glpi: glpi::Client::new(
            &cfg.glpi_url,
            cfg.client_id,
            cfg.client_secret,
            cfg.username,
            cfg.password,
        ),
        stale_after_hours: 26,
        // Configured by path only; whether the files are actually readable is
        // discovered per request and reported, not probed at startup. The
        // directory is 0750 glpi:glpi, so a deployment can be correctly
        // configured here and still be refused by the kernel - and that is
        // the case the Logs page most needs to state plainly.
        logs: cfg.log_dir.as_deref().map(|d| {
            tracing::info!("GLPI log directory configured for the Logs page: {d}");
            logs::LogDir::new(d)
        }),
    });

    let app = api::router(state).layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&cfg.bind)
        .await
        .with_context(|| format!("binding {}", cfg.bind))?;
    tracing::info!("listening on http://{}", cfg.bind);

    axum::serve(listener, app).await.context("serving")?;
    Ok(())
}

/// Candidate bundle directories, in preference order, for a backend binary
/// living at `exe`.
///
/// Derived from the binary's own path, not the process working directory.
/// The working directory is the wrong thing to ask: it is the project root
/// under `just run` and $HOME under a systemd user unit, which has no
/// WorkingDirectory and no reason to need one. The old CWD-relative guess
/// therefore worked in development and silently failed in deployment,
/// looking for ~/target/dx/... , finding nothing, and serving the API with
/// no page - a 404 that reads as a broken frontend build rather than as a
/// path that was never going to resolve.
///
/// Both profiles are offered because the pairing can legitimately differ.
/// The profile of the running binary comes first: a release backend normally
/// ships with a release bundle. The other profile follows rather than being
/// rejected, because a debug bundle left over from `just build-frontend` is
/// a working dashboard, and refusing it would serve nothing out of tidiness.
fn bundle_candidates(exe: &Path) -> Vec<PathBuf> {
    // <root>/target/<profile>/glpi-inv-backend -> <root>/target
    let Some(target) = exe.parent().and_then(Path::parent) else {
        return Vec::new();
    };
    let profile = exe
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("debug");
    let fallback = if profile == "release" {
        "debug"
    } else {
        "release"
    };

    [profile, fallback]
        .iter()
        .map(|p| {
            target
                .join("dx/glpi-inv-frontend")
                .join(p)
                .join("web/public")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::bundle_candidates;
    use std::path::{Path, PathBuf};

    fn candidates(exe: &str) -> Vec<PathBuf> {
        bundle_candidates(Path::new(exe))
    }

    /// The deployment case: the unit runs the release binary, so the release
    /// bundle has to be looked for first - and looked for at all, which the
    /// CWD-relative guess never did.
    #[test]
    fn a_release_binary_prefers_the_release_bundle() {
        let got = candidates("/home/pde/Projects/glpi-inv/target/release/glpi-inv-backend");
        assert_eq!(
            got[0],
            PathBuf::from(
                "/home/pde/Projects/glpi-inv/target/dx/glpi-inv-frontend/release/web/public"
            )
        );
        assert!(got[1].ends_with("dx/glpi-inv-frontend/debug/web/public"));
    }

    #[test]
    fn a_debug_binary_prefers_the_debug_bundle() {
        let got = candidates("/home/pde/Projects/glpi-inv/target/debug/glpi-inv-backend");
        assert!(got[0].ends_with("dx/glpi-inv-frontend/debug/web/public"));
        assert!(got[1].ends_with("dx/glpi-inv-frontend/release/web/public"));
    }

    /// Paths are absolute here, so the candidates must be too - a relative
    /// result would reintroduce the dependency on the working directory that
    /// this function exists to remove.
    #[test]
    fn candidates_are_absolute_when_the_binary_path_is() {
        assert!(candidates("/opt/glpi-inv/target/release/glpi-inv-backend")
            .iter()
            .all(|p| p.is_absolute()));
    }

    /// A binary with nothing above it yields nothing, rather than a path
    /// rooted somewhere arbitrary.
    #[test]
    fn a_binary_with_no_grandparent_yields_nothing() {
        assert!(candidates("glpi-inv-backend").is_empty());
    }
}
