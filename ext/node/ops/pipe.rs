// Copyright 2018-2025 the Deno authors. MIT license.

//! Windows named pipe support for Node.js compatibility.
//!
//! This module provides ops for connecting to and listening on Windows named pipes,
//! enabling Node.js packages that use named pipes to work on Windows in Deno.

#[cfg(windows)]
pub use windows::*;

#[cfg(windows)]
mod windows {
  use std::cell::Cell;
  use std::cell::RefCell;
  use std::rc::Rc;

  use deno_core::AsyncRefCell;
  use deno_core::AsyncResult;
  use deno_core::BufMutView;
  use deno_core::BufView;
  use deno_core::CancelHandle;
  use deno_core::CancelTryFuture;
  use deno_core::OpState;
  use deno_core::RcRef;
  use deno_core::Resource;
  use deno_core::ResourceId;
  use deno_core::op2;
  use deno_error::JsErrorBox;
  use tokio::io::AsyncReadExt;
  use tokio::io::AsyncWriteExt;
  use tokio::net::windows::named_pipe::ClientOptions;
  use tokio::net::windows::named_pipe::NamedPipeClient;
  use tokio::net::windows::named_pipe::NamedPipeServer;
  use tokio::net::windows::named_pipe::ServerOptions;

  #[derive(Debug, thiserror::Error, deno_error::JsError)]
  pub enum PipeError {
    #[class(generic)]
    #[error("{0}")]
    Io(std::io::Error),
    #[class("NotFound")]
    #[error("{0}")]
    NotFound(std::io::Error),
    #[class(inherit)]
    #[error(transparent)]
    Resource(#[from] deno_core::error::ResourceError),
    #[class(inherit)]
    #[error(transparent)]
    Canceled(#[from] deno_core::Canceled),
    #[class(generic)]
    #[error("Pipe connection failed: {0}")]
    ConnectionFailed(String),
  }

  impl From<std::io::Error> for PipeError {
    fn from(err: std::io::Error) -> Self {
      // Map various "not found" type errors to NotFound
      // On Windows, invalid pipe names can return different error codes
      match err.kind() {
        std::io::ErrorKind::NotFound => PipeError::NotFound(err),
        _ => {
          // Check raw OS error for Windows-specific codes
          if let Some(os_err) = err.raw_os_error() {
            match os_err {
              2 => return PipeError::NotFound(err),   // ERROR_FILE_NOT_FOUND
              3 => return PipeError::NotFound(err),   // ERROR_PATH_NOT_FOUND
              123 => return PipeError::NotFound(err), // ERROR_INVALID_NAME
              _ => {}
            }
          }
          PipeError::Io(err)
        }
      }
    }
  }

  /// Resource representing a Windows named pipe client connection.
  pub struct NamedPipeClientResource {
    pipe: AsyncRefCell<Option<NamedPipeClient>>,
    closed: Cell<bool>,
    cancel: CancelHandle,
  }

  impl Resource for NamedPipeClientResource {
    fn name(&self) -> std::borrow::Cow<'_, str> {
      "namedPipeClient".into()
    }

    fn close(self: Rc<Self>) {
      self.closed.set(true);
      self.cancel.cancel();
    }

    fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
      Box::pin(async move {
        if self.closed.get() {
          return Ok(BufView::from(vec![]));
        }
        let mut data = vec![0u8; limit];
        let cancel = RcRef::map(&self, |r| &r.cancel);
        let mut pipe_guard = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
        if self.closed.get() {
          let _ = pipe_guard.take();
          return Ok(BufView::from(vec![]));
        }
        let Some(pipe) = pipe_guard.as_mut() else {
          return Ok(BufView::from(vec![]));
        };
        let read_result = pipe.read(&mut data).try_or_cancel(cancel).await;
        if self.closed.get() {
          let _ = pipe_guard.take();
          return Ok(BufView::from(vec![]));
        }
        let nread = match read_result {
          Ok(n) => n,
          Err(e) => {
            let _ = pipe_guard.take();
            // Check for cancellation (ConnectionAborted is used when cancelled)
            if e.kind() == std::io::ErrorKind::ConnectionAborted {
              return Ok(BufView::from(vec![]));
            }
            if e.kind() == std::io::ErrorKind::BrokenPipe {
              return Ok(BufView::from(vec![]));
            }
            return Err(JsErrorBox::from_err(e));
          }
        };
        data.truncate(nread);
        Ok(BufView::from(data))
      })
    }

    fn read_byob(
      self: Rc<Self>,
      mut buf: BufMutView,
    ) -> AsyncResult<(usize, BufMutView)> {
      Box::pin(async move {
        if self.closed.get() {
          return Ok((0, buf));
        }
        let cancel = RcRef::map(&self, |r| &r.cancel);
        let mut pipe_guard = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
        if self.closed.get() {
          let _ = pipe_guard.take();
          return Ok((0, buf));
        }
        let Some(pipe) = pipe_guard.as_mut() else {
          return Ok((0, buf));
        };
        let read_result = pipe.read(&mut buf).try_or_cancel(cancel).await;
        if self.closed.get() {
          let _ = pipe_guard.take();
          return Ok((0, buf));
        }
        let nread = match read_result {
          Ok(n) => n,
          Err(e) => {
            let _ = pipe_guard.take();
            if e.kind() == std::io::ErrorKind::ConnectionAborted {
              return Ok((0, buf));
            }
            if e.kind() == std::io::ErrorKind::BrokenPipe {
              return Ok((0, buf));
            }
            return Err(JsErrorBox::from_err(e));
          }
        };
        Ok((nread, buf))
      })
    }

    fn write(self: Rc<Self>, buf: BufView) -> AsyncResult<deno_core::WriteOutcome> {
      Box::pin(async move {
        if self.closed.get() {
          return Ok(deno_core::WriteOutcome::Full { nwritten: 0 });
        }
        let cancel = RcRef::map(&self, |r| &r.cancel);
        let mut pipe_guard = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
        if self.closed.get() {
          let _ = pipe_guard.take();
          return Ok(deno_core::WriteOutcome::Full { nwritten: 0 });
        }
        let Some(pipe) = pipe_guard.as_mut() else {
          return Ok(deno_core::WriteOutcome::Full { nwritten: 0 });
        };
        let nwritten = buf.len();
        let write_result = pipe.write_all(&buf).try_or_cancel(cancel).await;
        match write_result {
          Ok(()) => {}
          Err(e) => {
            let _ = pipe_guard.take();
            if e.kind() == std::io::ErrorKind::ConnectionAborted {
              return Ok(deno_core::WriteOutcome::Full { nwritten: 0 });
            }
            if e.kind() == std::io::ErrorKind::BrokenPipe {
              return Ok(deno_core::WriteOutcome::Full { nwritten: 0 });
            }
            return Err(JsErrorBox::from_err(e));
          }
        };
        let _ = pipe.flush().await;
        if self.closed.get() {
          let _ = pipe_guard.take();
        }
        Ok(deno_core::WriteOutcome::Full { nwritten })
      })
    }

    fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
      Box::pin(async move {
        self.closed.set(true);
        self.cancel.cancel();
        let mut pipe_guard = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
        let _ = pipe_guard.take();
        Ok(())
      })
    }
  }

  impl NamedPipeClientResource {
    pub fn new(pipe: NamedPipeClient) -> Self {
      Self {
        pipe: AsyncRefCell::new(Some(pipe)),
        closed: Cell::new(false),
        cancel: CancelHandle::new(),
      }
    }
  }

  /// Resource representing a Windows named pipe server.
  pub struct NamedPipeServerResource {
    name: String,
    server: AsyncRefCell<Option<NamedPipeServer>>,
    cancel: CancelHandle,
  }

  impl Resource for NamedPipeServerResource {
    fn name(&self) -> std::borrow::Cow<'_, str> {
      "namedPipeServer".into()
    }

    fn close(self: Rc<Self>) {
      self.cancel.cancel();
    }
  }

  impl NamedPipeServerResource {
    pub fn new(name: String, server: NamedPipeServer) -> Self {
      Self {
        name,
        server: AsyncRefCell::new(Some(server)),
        cancel: CancelHandle::new(),
      }
    }
  }

  /// Connect to a Windows named pipe as a client.
  #[op2(async)]
  #[smi]
  pub async fn op_node_pipe_connect(
    state: Rc<RefCell<OpState>>,
    #[string] name: String,
  ) -> Result<ResourceId, PipeError> {
    let pipe = loop {
      match ClientOptions::new().open(&name) {
        Ok(pipe) => break pipe,
        Err(e) if e.raw_os_error() == Some(231) => {
          // ERROR_PIPE_BUSY (231) - All pipe instances are busy
          tokio::time::sleep(std::time::Duration::from_millis(50)).await;
          continue;
        }
        Err(e) => return Err(PipeError::Io(e)),
      }
    };

    let resource = NamedPipeClientResource::new(pipe);
    let rid = state.borrow_mut().resource_table.add(resource);
    Ok(rid)
  }

  /// Create a Windows named pipe server and start listening.
  #[op2(fast)]
  #[smi]
  pub fn op_node_pipe_listen(
    state: &mut OpState,
    #[string] name: String,
  ) -> Result<ResourceId, PipeError> {
    // Don't set max_instances to get unlimited instances (default behavior)
    let server = ServerOptions::new()
      .first_pipe_instance(true)
      .create(&name)?;

    let resource = NamedPipeServerResource::new(name, server);
    let rid = state.resource_table.add(resource);
    Ok(rid)
  }

  /// Wait for a client to connect to the named pipe server.
  #[op2(async)]
  #[smi]
  pub async fn op_node_pipe_accept(
    state: Rc<RefCell<OpState>>,
    #[smi] rid: ResourceId,
  ) -> Result<ResourceId, PipeError> {
    let resource = state
      .borrow()
      .resource_table
      .get::<NamedPipeServerResource>(rid)?;

    let cancel = RcRef::map(&resource, |r| &r.cancel);
    let name = resource.name.clone();

    let mut server_cell =
      RcRef::map(&resource, |r| &r.server).borrow_mut().await;
    let server = server_cell.take().ok_or_else(|| {
      PipeError::ConnectionFailed("Server already accepting".to_string())
    })?;

    let connect_result = server.connect().try_or_cancel(cancel).await;

    match connect_result {
      Ok(()) => {
        // Create a new server instance for the next connection
        let new_server = ServerOptions::new().create(&name)?;
        *server_cell = Some(new_server);

        let client_resource = NamedPipeServerConnectionResource::new(server);
        let client_rid = state.borrow_mut().resource_table.add(client_resource);
        Ok(client_rid)
      }
      Err(e) => {
        *server_cell = Some(server);
        Err(PipeError::Io(e))
      }
    }
  }

  /// Resource representing a connected Windows named pipe (from server side).
  pub struct NamedPipeServerConnectionResource {
    pipe: AsyncRefCell<Option<NamedPipeServer>>,
    closed: Cell<bool>,
    cancel: CancelHandle,
  }

  impl Resource for NamedPipeServerConnectionResource {
    fn name(&self) -> std::borrow::Cow<'_, str> {
      "namedPipeServerConnection".into()
    }

    fn close(self: Rc<Self>) {
      self.closed.set(true);
      self.cancel.cancel();
    }

    fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
      Box::pin(async move {
        if self.closed.get() {
          return Ok(BufView::from(vec![]));
        }
        let mut data = vec![0u8; limit];
        let cancel = RcRef::map(&self, |r| &r.cancel);
        let mut pipe_guard = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
        if self.closed.get() {
          let _ = pipe_guard.take();
          return Ok(BufView::from(vec![]));
        }
        let Some(pipe) = pipe_guard.as_mut() else {
          return Ok(BufView::from(vec![]));
        };
        let read_result = pipe.read(&mut data).try_or_cancel(cancel).await;
        if self.closed.get() {
          let _ = pipe_guard.take();
          return Ok(BufView::from(vec![]));
        }
        let nread = match read_result {
          Ok(n) => n,
          Err(e) => {
            let _ = pipe_guard.take();
            if e.kind() == std::io::ErrorKind::ConnectionAborted {
              return Ok(BufView::from(vec![]));
            }
            if e.kind() == std::io::ErrorKind::BrokenPipe {
              return Ok(BufView::from(vec![]));
            }
            return Err(JsErrorBox::from_err(e));
          }
        };
        data.truncate(nread);
        Ok(BufView::from(data))
      })
    }

    fn read_byob(
      self: Rc<Self>,
      mut buf: BufMutView,
    ) -> AsyncResult<(usize, BufMutView)> {
      Box::pin(async move {
        if self.closed.get() {
          return Ok((0, buf));
        }
        let cancel = RcRef::map(&self, |r| &r.cancel);
        let mut pipe_guard = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
        if self.closed.get() {
          let _ = pipe_guard.take();
          return Ok((0, buf));
        }
        let Some(pipe) = pipe_guard.as_mut() else {
          return Ok((0, buf));
        };
        let read_result = pipe.read(&mut buf).try_or_cancel(cancel).await;
        if self.closed.get() {
          let _ = pipe_guard.take();
          return Ok((0, buf));
        }
        let nread = match read_result {
          Ok(n) => n,
          Err(e) => {
            let _ = pipe_guard.take();
            if e.kind() == std::io::ErrorKind::ConnectionAborted {
              return Ok((0, buf));
            }
            if e.kind() == std::io::ErrorKind::BrokenPipe {
              return Ok((0, buf));
            }
            return Err(JsErrorBox::from_err(e));
          }
        };
        Ok((nread, buf))
      })
    }

    fn write(self: Rc<Self>, buf: BufView) -> AsyncResult<deno_core::WriteOutcome> {
      Box::pin(async move {
        if self.closed.get() {
          return Ok(deno_core::WriteOutcome::Full { nwritten: 0 });
        }
        let cancel = RcRef::map(&self, |r| &r.cancel);
        let mut pipe_guard = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
        if self.closed.get() {
          let _ = pipe_guard.take();
          return Ok(deno_core::WriteOutcome::Full { nwritten: 0 });
        }
        let Some(pipe) = pipe_guard.as_mut() else {
          return Ok(deno_core::WriteOutcome::Full { nwritten: 0 });
        };
        let nwritten = buf.len();
        let write_result = pipe.write_all(&buf).try_or_cancel(cancel).await;
        match write_result {
          Ok(()) => {}
          Err(e) => {
            let _ = pipe_guard.take();
            if e.kind() == std::io::ErrorKind::ConnectionAborted {
              return Ok(deno_core::WriteOutcome::Full { nwritten: 0 });
            }
            if e.kind() == std::io::ErrorKind::BrokenPipe {
              return Ok(deno_core::WriteOutcome::Full { nwritten: 0 });
            }
            return Err(JsErrorBox::from_err(e));
          }
        };
        let _ = pipe.flush().await;
        if self.closed.get() {
          let _ = pipe_guard.take();
        }
        Ok(deno_core::WriteOutcome::Full { nwritten })
      })
    }

    fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
      Box::pin(async move {
        self.closed.set(true);
        self.cancel.cancel();
        let mut pipe_guard = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
        let _ = pipe_guard.take();
        Ok(())
      })
    }
  }

  impl NamedPipeServerConnectionResource {
    pub fn new(pipe: NamedPipeServer) -> Self {
      Self {
        pipe: AsyncRefCell::new(Some(pipe)),
        closed: Cell::new(false),
        cancel: CancelHandle::new(),
      }
    }
  }
}

// Stub ops for non-Windows platforms
#[cfg(not(windows))]
pub use stubs::*;

#[cfg(not(windows))]
mod stubs {
  use std::cell::RefCell;
  use std::rc::Rc;

  use deno_core::OpState;
  use deno_core::ResourceId;
  use deno_core::op2;

  #[derive(Debug, thiserror::Error, deno_error::JsError)]
  #[class(generic)]
  #[error("Windows named pipes are not supported on this platform")]
  pub struct NotSupportedError;

  #[allow(clippy::unused_async)]
  #[op2(async)]
  #[smi]
  pub async fn op_node_pipe_connect(
    _state: Rc<RefCell<OpState>>,
    #[string] _name: String,
  ) -> Result<ResourceId, NotSupportedError> {
    Err(NotSupportedError)
  }

  #[op2(fast)]
  #[smi]
  pub fn op_node_pipe_listen(
    _state: &mut OpState,
    #[string] _name: String,
  ) -> Result<ResourceId, NotSupportedError> {
    Err(NotSupportedError)
  }

  #[allow(clippy::unused_async)]
  #[op2(async)]
  #[smi]
  pub async fn op_node_pipe_accept(
    _state: Rc<RefCell<OpState>>,
    #[smi] _rid: ResourceId,
  ) -> Result<ResourceId, NotSupportedError> {
    Err(NotSupportedError)
  }
}
