mod error;
mod parse;
mod types;
pub mod convert;
pub mod defs;
pub mod flags;
pub mod completions;
pub mod help;

pub use error::CliError;
pub use error::CliErrorKind;
pub use parse::parse;
pub use types::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_full;
