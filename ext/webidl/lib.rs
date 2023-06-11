// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

deno_core::extension!(
  deno_webidl,
  esm = ["00_webidl.js"],
  exclude_js_sources_cfg = (all(
    feature = "exclude_js_sources",
    not(feature = "force_include_js_sources")
  )),
);
