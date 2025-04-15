// Copyright 2018-2025 the Deno authors. MIT license.

mod rust_list;

pub use rust_list::UNSTABLE_FLAGS;

pub const JS_SOURCE: deno_core::FastStaticString =
  deno_core::ascii_str_include!("./js_list.js");
