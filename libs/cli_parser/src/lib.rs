// Copyright 2018-2026 the Deno authors. MIT license.

// `flags` holds the canonical flag type definitions (`Flags`,
// `DenoSubcommand`, etc.), shared by this crate and re-exported by the Deno CLI.
pub mod flags;

// The zero-cost argument parser. `types` defines the static `CommandDef` /
// `ArgDef` tables, `defs` is the actual command tree for Deno, `parse` walks
// argv against the tables into a `ParseResult`, `convert` turns that into
// `Flags`, and `help` / `completions` render help text and shell completions
// from the same static tables. `error` is the parser's public error type.
//
// None of this is wired into the CLI's parsing path yet (clap still parses in
// production); a later change does the cutover and adds the parity test suite.
pub mod completions;
pub mod convert;
pub mod defs;
mod error;
pub mod help;
mod parse;
mod types;

pub use error::CliError;
pub use error::CliErrorKind;
pub use parse::parse;
pub use types::*;

#[cfg(test)]
mod tests;
