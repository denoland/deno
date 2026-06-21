// Copyright 2018-2026 the Deno authors. MIT license.

// The canonical flag type definitions (`Flags`, `DenoSubcommand`, etc.) live
// here so they can be shared by the parser crate and re-exported by the Deno
// CLI.
pub mod flags;

// The zero-cost argument parser. `types` defines the static `CommandDef` /
// `ArgDef` tables, `parse` walks argv against them, and `error` is the parser's
// public error type. These are not yet wired into the CLI's parsing path; that
// happens in a later change once the static command definitions and the
// `ParseResult` -> `Flags` conversion land.
mod error;
mod parse;
mod types;

pub use error::CliError;
pub use error::CliErrorKind;
pub use parse::parse;
pub use types::*;

#[cfg(test)]
mod tests;
