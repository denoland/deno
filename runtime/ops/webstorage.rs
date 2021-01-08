// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;
use deno_webstorage::LocationDataDir;
use deno_webstorage::op_localstorage_open;
use deno_webstorage::op_localstorage_length;
use deno_webstorage::op_localstorage_key;
use deno_webstorage::op_localstorage_set;
use deno_webstorage::op_localstorage_get;
use deno_webstorage::op_localstorage_remove;
use deno_webstorage::op_localstorage_clear;

pub fn init(rt: &mut deno_core::JsRuntime, deno_dir: Option<PathBuf>) {
  {
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put::<LocationDataDir>(LocationDataDir(deno_dir));
  }
  super::reg_json_sync(rt, "op_localstorage_open", op_localstorage_open);
  super::reg_json_sync(rt, "op_localstorage_length", op_localstorage_length);
  super::reg_json_sync(rt, "op_localstorage_key", op_localstorage_key);
  super::reg_json_sync(rt, "op_localstorage_set", op_localstorage_set);
  super::reg_json_sync(rt, "op_localstorage_get", op_localstorage_get);
  super::reg_json_sync(rt, "op_localstorage_remove", op_localstorage_remove);
  super::reg_json_sync(rt, "op_localstorage_clear", op_localstorage_clear);
}
