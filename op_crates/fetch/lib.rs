// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::js_check;
use deno_core::JsRuntime;
use std::path::Path;

pub fn init(isolate: &mut JsRuntime) {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let files = vec![manifest_dir.join("11_streams.js")];
  // TODO(nayeemrmn): https://github.com/rust-lang/cargo/issues/3946 to get the
  // workspace root.
  let display_root = manifest_dir.parent().unwrap().parent().unwrap();
  for file in files {
    println!("cargo:rerun-if-changed={}", file.display());
    let display_path = file.strip_prefix(display_root).unwrap();
    let display_path_str = display_path.display().to_string();
    js_check(isolate.execute(
      &("deno:".to_string() + &display_path_str.replace('\\', "/")),
      &std::fs::read_to_string(&file).unwrap(),
    ));
  }
}
