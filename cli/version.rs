// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use serde_json;
pub const DENO: &str = env!("CARGO_PKG_VERSION");

pub fn v8() -> &'static str {
  deno::v8_version()
}

pub fn typescript() -> String {
  // TODO: By using include_str! we are including the package.json into
  // the deno binary using serde to decode it at runtime. This is suboptimal
  // in space and time. We need to extract the TypeScript version at compile
  // time instead. This will be easier after #2608.
  let data = include_str!("../node_modules/typescript/package.json");
  let pkg: serde_json::Value = serde_json::from_str(data).unwrap();
  pkg["version"].as_str().unwrap().to_string()
}
