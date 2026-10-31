use std::sync::Arc;

use anyhow::{Context, Result};
use glpi_inv_backend::{api, config::Config, glpi};

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
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let guess = std::path::PathBuf::from("target/dx/glpi-inv-frontend/debug/web/public");
            guess.join("index.html").is_file().then_some(guess)
        });

    match &static_dir {
        Some(d) => tracing::info!("serving web bundle from {}", d.display()),
        None => {
            tracing::info!("no web bundle found; API only (build it with `just build-frontend`)")
        }
    }

    let state = Arc::new(api::AppState {
        static_dir,
        glpi: glpi::Client::new(
            &cfg.glpi_url,
            cfg.client_id,
            cfg.client_secret,
            cfg.username,
            cfg.password,
        ),
        stale_after_hours: 26,
    });

    let app = api::router(state).layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&cfg.bind)
        .await
        .with_context(|| format!("binding {}", cfg.bind))?;
    tracing::info!("listening on http://{}", cfg.bind);

    axum::serve(listener, app).await.context("serving")?;
    Ok(())
}
