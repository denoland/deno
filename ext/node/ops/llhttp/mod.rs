// Copyright 2018-2026 the Deno authors. MIT license.

//! Rust FFI bindings for llhttp — the HTTP/1.1 parser used by Node.js.
//!
//! llhttp is a push-based, callback-driven parser. Data is fed via
//! `llhttp_execute()` and the parser invokes callbacks synchronously
//! during parsing for headers, body chunks, and message boundaries.

#[allow(
  non_camel_case_types,
  non_upper_case_globals,
  dead_code,
  clippy::upper_case_acronyms
)]
pub mod sys;

pub use sys::*;

#[cfg(test)]
mod tests;
