//! glpi-inv backend.
//!
//! Reads agent telemetry from GLPI's High-Level API (v2.3) and serves it to
//! the Dioxus frontend. Deliberately read-only: it never writes to GLPI.
//!
//! Why the HL API and not GLPI's database: the API respects GLPI's own entity
//! permissions and is version-stable, while a direct MariaDB reader bypasses
//! permissions entirely and couples this app to GLPI's schema across
//! upgrades. The cost is real and documented - see `glpi::agents` and
//! CLAUDE.md for the two things the API does not expose.

pub mod api;
pub mod config;
pub mod glpi;
