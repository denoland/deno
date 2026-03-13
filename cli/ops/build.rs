// Copyright 2018-2026 the Deno authors. MIT license.

//! Ops extension for the bundler plugin host.
//!
//! Currently a minimal extension — the primary plugin communication happens
//! via direct v8::Function calls from Rust → JS and return values back.
//! Ops can be added here in the future for things like async resolve/load
//! callbacks that need to call back into Deno's resolver from JS.

deno_core::extension!(
  deno_build_ext,
  // No ops needed yet — plugins communicate via direct v8::Function calls
  // and return values. Future ops can be added for async plugin callbacks
  // that need Rust-side resolution (e.g., calling into CliResolver from JS).
);
