// Copyright 2018-2026 the Deno authors. MIT license.

// The canonical flag type definitions (`Flags`, `DenoSubcommand`, etc.) live
// here so they can be shared by the parser crate and re-exported by the Deno
// CLI. Parsing itself currently still lives in `cli/args/flags.rs`; this crate
// will grow the zero-cost parser in subsequent changes.
pub mod flags;
