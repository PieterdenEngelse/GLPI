//! `glpi-cli <path>` - GET a GLPI High-Level API v2.3 path and print the JSON.
//!
//! Exists so the API can be inspected without the backend or the browser in
//! the way. When a dashboard shows nothing, this answers "is it GLPI or is it
//! us" in one command.
//!
//!   glpi-cli Inventory/Agent
//!   glpi-cli Assets/Computer
use anyhow::{Context, Result};
use glpi_inv_backend::{config::Config, glpi};

#[tokio::main]
async fn main() -> Result<()> {
    let path = std::env::args().nth(1).context(
        "usage: glpi-cli <api-path>   e.g. glpi-cli Inventory/Agent\n\
         needs GLPI_CLIENT_ID and GLPI_CLIENT_SECRET in the environment",
    )?;

    let cfg = Config::from_env()?;
    let client = glpi::Client::new(
        &cfg.glpi_url,
        cfg.client_id,
        cfg.client_secret,
        cfg.username,
        cfg.password,
    );
    let value = client.get(&path).await?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
