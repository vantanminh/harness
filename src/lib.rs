#![forbid(unsafe_code)]

pub mod app;
pub mod cli;
pub mod domain;
pub mod error;
pub mod infra;

pub use error::{Error, Result};

/// Package semver — kept in lockstep with `package.json` via bump-version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() -> Result<()> {
    cli::run()
}
