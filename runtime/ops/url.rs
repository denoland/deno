// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno_url::op_url_parse;
use deno_url::op_url_parse_search_params;
use deno_url::op_url_stringify_search_params;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_sync(rt, "op_url_parse", op_url_parse);
  super::reg_sync(rt, "op_url_parse_search_params", op_url_parse_search_params);
  super::reg_sync(
    rt,
    "op_url_stringify_search_params",
    op_url_stringify_search_params,
  );
}
