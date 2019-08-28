// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{wrap_json_op, JsonOp};
use crate::resources::table_entries;
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use deno::*;

// Resources

pub struct OpResources;

impl DenoOpDispatcher for OpResources {
  fn dispatch(
    &self,
    _state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |_args, _zero_copy| {
        let serialized_resources = table_entries();
        Ok(JsonOp::Sync(json!(serialized_resources)))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "resources";
}
