// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![allow(unused_mut)]
#![allow(non_camel_case_types)]
#![allow(clippy::undocumented_unsafe_blocks)]

//! Symbols to be exported are now defined in this JSON file.
//! The `#[napi_sym]` macro checks for missing entries and panics.
//!
//! `./tools/napi/generate_symbols_list.js` is used to generate the LINK `cli/exports.def` on Windows,
//! which is also checked into git.
//!
//! To add a new napi function:
//! 1. Place `#[napi_sym]` on top of your implementation.
//! 2. Add the function's identifier to this JSON list.
//! 3. Finally, run `tools/napi/generate_symbols_list.js` to update `cli/napi/generated_symbol_exports_list_*.def`.

pub mod js_native_api;
pub mod node_api;
pub mod util;
pub mod uv;
