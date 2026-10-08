//! PerfectUninstaller core: cross-platform uninstall engine.
//!
//! Flow: [`Engine::scan`] discovers installed software, [`Engine::analyze`]
//! builds a [`RemovalPlan`] of every residual trace, [`Engine::uninstall`]
//! permanently deletes the selected paths behind safety guards and an audit log.

pub mod engine;
pub mod error;
pub mod models;
pub mod platform;
pub mod safety;
pub mod trace_search;
pub mod util;

pub use engine::Engine;
pub use error::{Error, Result};
pub use models::*;
