// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::AsyncRefCell;
use deno_core::BufView;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceHandle;
use deno_core::ResourceId;
use deno_core::WriteOutcome;
use deno_core::op2;
use deno_error::JsErrorBox;
use futures::FutureExt;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::DuplexStream;
use tokio::io::ReadHalf;
use tokio::io::WriteHalf;

struct PipeResource {
  tx: AsyncRefCell<WriteHalf<DuplexStream>>,
  rx: AsyncRefCell<ReadHalf<DuplexStream>>,
}

impl Resource for PipeResource {
  fn read_byob(
    self: std::rc::Rc<Self>,
    mut buf: deno_core::BufMutView,
  ) -> deno_core::AsyncResult<(usize, deno_core::BufMutView)> {
    async {
      let mut lock = RcRef::map(self, |this| &this.rx).borrow_mut().await;
      // Note that we're holding a slice across an await point, so this code is very much not safe
      let res = lock.read(&mut buf).await.map_err(JsErrorBox::from_err)?;
      Ok((res, buf))
    }
    .boxed_local()
  }

  fn write(
    self: std::rc::Rc<Self>,
    buf: BufView,
  ) -> deno_core::AsyncResult<deno_core::WriteOutcome> {
    async {
      let mut lock = RcRef::map(self, |this| &this.tx).borrow_mut().await;
      let nwritten = lock.write(&buf).await.map_err(JsErrorBox::from_err)?;
      Ok(WriteOutcome::Partial {
        nwritten,
        view: buf,
      })
    }
    .boxed_local()
  }
}

#[op2]
#[serde]
pub fn op_pipe_create(op_state: &mut OpState) -> (ResourceId, ResourceId) {
  let (s1, s2) = tokio::io::duplex(1024);
  let (rx1, tx1) = tokio::io::split(s1);
  let (rx2, tx2) = tokio::io::split(s2);
  let rid1 = op_state.resource_table.add(PipeResource {
    rx: AsyncRefCell::new(rx1),
    tx: AsyncRefCell::new(tx1),
  });
  let rid2 = op_state.resource_table.add(PipeResource {
    rx: AsyncRefCell::new(rx2),
    tx: AsyncRefCell::new(tx2),
  });
  (rid1, rid2)
}

struct FileResource {
  handle: deno_core::ResourceHandle,
}

impl FileResource {
  fn new(file: tokio::fs::File) -> Self {
    let handle = ResourceHandle::from_fd_like(&file);
    Self { handle }
  }
}

impl Resource for FileResource {
  fn backing_handle(self: Rc<Self>) -> Option<ResourceHandle> {
    Some(self.handle)
  }

  fn read_byob(
    self: std::rc::Rc<Self>,
    buf: deno_core::BufMutView,
  ) -> deno_core::AsyncResult<(usize, deno_core::BufMutView)> {
    async {
      // Do something to test unrefing.
      tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
      Ok((0, buf))
    }
    .boxed_local()
  }
}

#[op2]
#[serde]
pub async fn op_file_open(
  #[string] path: String,
  ref_: bool,
  op_state: Rc<RefCell<OpState>>,
) -> Result<ResourceId, std::io::Error> {
  let tokio_file = tokio::fs::OpenOptions::new()
    .read(true)
    .write(false)
    .create(false)
    .open(&path)
    .await?;
  let rid = op_state
    .borrow_mut()
    .resource_table
    .add(FileResource::new(tokio_file));

  if !ref_ {
    op_state.borrow_mut().uv_unref(rid);
  }

  Ok(rid)
}

#[op2]
#[string]
pub fn op_path_to_url(#[string] path: &str) -> Result<String, std::io::Error> {
  let path = std::path::absolute(path)?;
  let url = url::Url::from_file_path(path).unwrap();
  Ok(url.to_string())
}
