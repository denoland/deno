// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error;
use crate::msg;
use crate::ops::empty_buf;
use crate::ops::ok_buf;
use crate::ops::serialize_response;
use crate::ops::CliOpResult;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use futures::Future;

pub fn op_cache(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_cache().unwrap();
  let extension = inner.extension().unwrap();
  // TODO: rename to something with 'url'
  let module_id = inner.module_id().unwrap();
  let contents = inner.contents().unwrap();

  let module_specifier = ModuleSpecifier::resolve_url(module_id)
    .expect("Should be valid module specifier");

  state.ts_compiler.cache_compiler_output(
    &module_specifier,
    extension,
    contents,
  )?;

  ok_buf(empty_buf())
}

pub fn op_fetch_source_file(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  if !base.sync() {
    return Err(deno_error::no_async_support());
  }
  assert!(data.is_none());
  let inner = base.inner_as_fetch_source_file().unwrap();
  let cmd_id = base.cmd_id();
  let specifier = inner.specifier().unwrap();
  let referrer = inner.referrer().unwrap();

  // TODO(ry) Maybe a security hole. Only the compiler worker should have access
  // to this. Need a test to demonstrate the hole.
  let is_dyn_import = false;

  let resolved_specifier =
    state.resolve(specifier, referrer, false, is_dyn_import)?;

  let fut = state
    .file_fetcher
    .fetch_source_file_async(&resolved_specifier)
    .and_then(move |out| {
      let builder = &mut FlatBufferBuilder::new();
      let data_off = builder.create_vector(out.source_code.as_slice());
      let msg_args = msg::FetchSourceFileResArgs {
        module_name: Some(builder.create_string(&out.url.to_string())),
        filename: Some(builder.create_string(&out.filename.to_str().unwrap())),
        media_type: out.media_type,
        data: Some(data_off),
      };
      let inner = msg::FetchSourceFileRes::create(builder, &msg_args);
      Ok(serialize_response(
        cmd_id,
        builder,
        msg::BaseArgs {
          inner: Some(inner.as_union_value()),
          inner_type: msg::Any::FetchSourceFileRes,
          ..Default::default()
        },
      ))
    });

  // WARNING: Here we use tokio_util::block_on() which starts a new Tokio
  // runtime for executing the future. This is so we don't inadvernently run
  // out of threads in the main runtime.
  let result_buf = tokio_util::block_on(fut)?;
  Ok(Op::Sync(result_buf))
}
