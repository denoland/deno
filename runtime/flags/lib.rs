// Copyright 2018-2025 the Deno authors. MIT license.

include!(concat!(env!("OUT_DIR"), "/rust_list.rs"));

pub const JS_SOURCE: deno_core::FastStaticString =
  deno_core::ascii_str_include!(concat!(env!("OUT_DIR"), "/js_list.js"));
