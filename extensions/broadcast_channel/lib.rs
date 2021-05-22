// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::AsyncRefCell;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

struct BroadcastChannelResource(AsyncRefCell<tokio::fs::File>);

impl Resource for BroadcastChannelResource {
  fn name(&self) -> Cow<str> {
    "broadcastChannel".into()
  }
}

pub fn op_broadcast_open(
  state: &mut OpState,
  name: String,
  _bufs: Option<ZeroCopyBuf>,
) -> Result<ResourceId, AnyError> {
  let path = PathBuf::from("./");
  std::fs::create_dir_all(&path)?;
  let file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .read(true)
    .open(path.join(format!("broadcast_{}", name)))?;

  let rid =
    state
      .resource_table
      .add(BroadcastChannelResource(AsyncRefCell::new(
        tokio::fs::File::from_std(file),
      )));

  Ok(rid)
}

pub async fn op_broadcast_send(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let state = state.borrow_mut();
  let resource = state
    .resource_table
    .get::<BroadcastChannelResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let mut file = RcRef::map(&resource, |r| &r.0).borrow_mut().await;

  let buffer_data = buf.unwrap();
  let mut data = vec![];
  data.extend_from_slice(&(buffer_data.len() as u64).to_ne_bytes());
  data.extend_from_slice(&buffer_data);

  file.write_all(&data).await?;

  Ok(())
}

pub async fn op_broadcast_next_event(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _bufs: Option<ZeroCopyBuf>,
) -> Result<Vec<u8>, AnyError> {
  let resource = {
    let state = state.borrow_mut();
    state
      .resource_table
      .get::<BroadcastChannelResource>(rid)
      .ok_or_else(bad_resource_id)?
  };

  let mut file = RcRef::map(&resource, |r| &r.0).borrow_mut().await;

  let size = match file.read_u64().await {
    Ok(s) => s,
    Err(e) => {
      return match e.kind() {
        deno_core::futures::io::ErrorKind::UnexpectedEof => Ok(vec![]),
        _ => Err(e.into()),
      }
    }
  };
  let mut data = vec![0u8; size as usize];
  match file.read_exact(&mut data).await {
    Ok(s) => s,
    Err(e) => {
      return match e.kind() {
        deno_core::futures::io::ErrorKind::UnexpectedEof => Ok(vec![]),
        _ => Err(e.into()),
      }
    }
  };

  Ok(data)
}

pub fn init() -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:extensions/broadcast_channel",
      "01_broadcast_channel.js",
    ))
    .ops(vec![
      ("op_broadcast_open", op_sync(op_broadcast_open)),
      ("op_broadcast_send", op_async(op_broadcast_send)),
      ("op_broadcast_next_event", op_async(op_broadcast_next_event)),
    ])
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("lib.deno_broadcast_channel.d.ts")
}
