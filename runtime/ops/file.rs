// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno_core::url::Url;
use deno_file::op_file_create_object_url;
use deno_file::op_file_revoke_object_url;
use deno_file::BlobUrlStore;
use deno_file::Location;

pub fn init(
  rt: &mut deno_core::JsRuntime,
  blob_url_store: BlobUrlStore,
  maybe_location: Option<Url>,
) {
  {
    let op_state = rt.op_state();
    let mut op_state = op_state.borrow_mut();
    op_state.put(blob_url_store);
    if let Some(location) = maybe_location {
      op_state.put(Location(location));
    }
  }
  super::reg_sync(rt, "op_file_create_object_url", op_file_create_object_url);
  super::reg_sync(rt, "op_file_revoke_object_url", op_file_revoke_object_url);
}
