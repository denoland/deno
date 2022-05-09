// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::spawn::ChildStatus;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::AsyncResult;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_core::{Extension, Resource};
use portable_pty::Child;
use portable_pty::MasterPty;
use portable_pty::{native_pty_system, CommandBuilder, PtySize, PtySystem};
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::io::Read;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

use crate::permissions::Permissions;

pub struct Unstable(pub bool);

pub struct PtyWrapper(pub Box<dyn PtySystem>);

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![op_pty_open::decl(), op_pty_wait::decl()])
    .state(move |state| {
      state.put(PtyWrapper(native_pty_system()));
      Ok(())
    })
    .build()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPtyArgs {
  cmd: String,
  args: Vec<String>,
  cwd: Option<String>,
  clear_env: bool,
  env: Vec<(String, String)>,
  rows: u16,
  columns: u16,
}

#[derive(Serialize)]
pub struct PtyChild {
  rid: ResourceId,
  pid: u32,
}

struct PtyResource {
  child: Box<dyn Child + Send + Sync>,
  master: Box<dyn MasterPty + Send>,
  reader: Arc<Mutex<Box<dyn Read + Send>>>,
  writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl Resource for PtyResource {
  fn name(&self) -> Cow<str> {
    "pty".into()
  }
  fn read(self: Rc<Self>, mut buf: ZeroCopyBuf) -> AsyncResult<usize> {
    let reader = self.reader.clone();
    Box::pin(async move {
      tokio::task::spawn_blocking(move || {
        let mut r = reader.lock().unwrap();
        r.read(&mut buf)
      })
      .await?
      .map_err(AnyError::from)
    })
  }

  fn write(self: Rc<Self>, mut buf: ZeroCopyBuf) -> AsyncResult<usize> {
    let writer = self.writer.clone();
    Box::pin(async move {
      tokio::task::spawn_blocking(move || {
        let mut w = writer.lock().unwrap();
        w.write(&mut buf)
      })
      .await?
      .map_err(AnyError::from)
    })
  }
}

#[op]
fn op_pty_open(
  state: &mut OpState,
  args: OpenPtyArgs,
) -> Result<PtyChild, AnyError> {
  super::check_unstable(state, "Deno.openPty");
  state.borrow_mut::<Permissions>().run.check(&args.cmd)?;

  let pair = state.borrow_mut::<PtyWrapper>().0.openpty(PtySize {
    pixel_height: 0,
    pixel_width: 0,
    cols: args.columns,
    rows: args.rows,
  })?;

  let master = pair.master;
  let slave = pair.slave;

  let mut builder = CommandBuilder::new(args.cmd);
  builder.args(args.args);
  if args.clear_env {
    builder.env_clear();
  }
  if let Some(cwd) = args.cwd {
    builder.cwd(cwd);
  }
  for (key, value) in args.env {
    builder.env(key, value);
  }

  let reader = Arc::new(Mutex::new(master.try_clone_reader()?));
  let writer = Arc::new(Mutex::new(master.try_clone_writer()?));
  let child = slave.spawn_command(builder)?;
  let pid = child.process_id().unwrap();
  let rid = state.resource_table.add(PtyResource {
    master,
    child,
    reader,
    writer,
  });

  Ok(PtyChild { rid, pid })
}

#[op]
async fn op_pty_wait(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<bool, AnyError> {
  let resource = state.borrow_mut().resource_table.take::<PtyResource>(rid)?;
  let mut child = Rc::try_unwrap(resource).ok().unwrap().child;
  tokio::task::spawn_blocking(move || -> Result<bool, AnyError> {
    Ok(child.wait()?.success())
  })
  .await?
}
