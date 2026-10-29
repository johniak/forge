//! Forge tooling core.
//!
//! Every developer-facing capability (`forge check`, `forge context`, …) lives
//! here as a library function returning structured, versioned reports. The CLI
//! and the MCP server are thin adapters over this crate (SPEC §8.1).

pub mod architecture;
pub mod check;
pub mod manifest;
pub mod process;
pub mod report;
