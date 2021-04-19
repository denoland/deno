// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_webstorage::op_webstorage_clear;
use deno_webstorage::op_webstorage_get;
use deno_webstorage::op_webstorage_iterate_keys;
use deno_webstorage::op_webstorage_key;
use deno_webstorage::op_webstorage_length;
use deno_webstorage::op_webstorage_open;
use deno_webstorage::op_webstorage_remove;
use deno_webstorage::op_webstorage_set;
use deno_webstorage::LocationDataDir;
use std::path::PathBuf;

pub fn init(rt: &mut deno_core::JsRuntime, deno_dir: Option<PathBuf>) {
  {
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put::<LocationDataDir>(LocationDataDir(deno_dir));
  }
  super::reg_sync(rt, "op_webstorage_open", op_webstorage_open);
  super::reg_sync(rt, "op_webstorage_length", op_webstorage_length);
  super::reg_sync(rt, "op_webstorage_key", op_webstorage_key);
  super::reg_sync(rt, "op_webstorage_set", op_webstorage_set);
  super::reg_sync(rt, "op_webstorage_get", op_webstorage_get);
  super::reg_sync(rt, "op_webstorage_remove", op_webstorage_remove);
  super::reg_sync(rt, "op_webstorage_clear", op_webstorage_clear);
  super::reg_sync(rt, "op_webstorage_iterate_keys", op_webstorage_iterate_keys);
}
