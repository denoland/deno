// Copyright 2018-2026 the Deno authors. MIT license.
pub mod completions;
pub mod convert;
pub mod defs;
mod error;
pub mod flags;
pub mod help;
mod parse;
mod types;

pub use error::CliError;
pub use error::CliErrorKind;
pub use parse::parse;
pub use types::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_full;
