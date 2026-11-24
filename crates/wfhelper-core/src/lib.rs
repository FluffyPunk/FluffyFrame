//! WFHelper core: everything the original app ran in Electron's main process.
//!
//! Layout mirrors the original repository:
//! - [`config`] ≈ `config/` (shared constants, settings, static game tables)
//! - [`services`] ≈ `services/` (data sources, parsers, trackers)
//!
//! Nothing here depends on the GUI; the app crate drives these from worker threads.

pub mod config;
pub mod services;

pub use anyhow::{Error, Result};
