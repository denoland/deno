// Copyright 2018-2026 the Deno authors. MIT license.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
  if let Ok(input) = std::str::from_utf8(data) {
    let _ = deno_npm::fast_registry_json::pluck_versions(input);
    let _ = deno_npm::fast_registry_json::pluck_packument_index(input);
  }
});
