// Copyright 2018-2025 the Deno authors. MIT license.

//! Windows named pipe support for Node.js compatibility.
//!
//! This module provides ops for connecting to and listening on Windows named pipes,
//! enabling Node.js packages that use named pipes to work on Windows in Deno.

#[cfg(windows)]
pub use windows::*;

#[cfg(windows)]
mod windows {
  use std::cell::RefCell;
  use std::rc::Rc;

  use deno_core::AsyncRefCell;
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
    Io(#[from] std::io::Error),
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

  /// Resource representing a Windows named pipe client connection.
  pub struct NamedPipeClientResource {
    pipe: AsyncRefCell<NamedPipeClient>,
    cancel: CancelHandle,
  }

  impl Resource for NamedPipeClientResource {
    fn name(&self) -> std::borrow::Cow<str> {
      "namedPipeClient".into()
    }

    fn close(self: Rc<Self>) {
      self.cancel.cancel();
    }

    deno_core::impl_readable_byob!();
    deno_core::impl_writable!();
  }

  impl NamedPipeClientResource {
    pub fn new(pipe: NamedPipeClient) -> Self {
      Self {
        pipe: AsyncRefCell::new(pipe),
        cancel: CancelHandle::new(),
      }
    }

    pub async fn read(
      self: Rc<Self>,
      data: &mut [u8],
    ) -> Result<usize, PipeError> {
      let mut pipe = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
      let cancel = RcRef::map(&self, |r| &r.cancel);
      Ok(pipe.read(data).try_or_cancel(cancel).await??)
    }

    pub async fn write(
      self: Rc<Self>,
      data: &[u8],
    ) -> Result<usize, PipeError> {
      let mut pipe = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
      let nwritten = pipe.write(data).await?;
      pipe.flush().await?;
      Ok(nwritten)
    }
  }

  /// Resource representing a Windows named pipe server.
  pub struct NamedPipeServerResource {
    name: String,
    server: AsyncRefCell<Option<NamedPipeServer>>,
    cancel: CancelHandle,
  }

  impl Resource for NamedPipeServerResource {
    fn name(&self) -> std::borrow::Cow<str> {
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
  ///
  /// Returns a resource ID for the connected pipe.
  #[op2(async)]
  #[smi]
  pub async fn op_node_pipe_connect(
    state: Rc<RefCell<OpState>>,
    #[string] name: String,
  ) -> Result<ResourceId, PipeError> {
    // Try to connect to the named pipe
    // We may need to retry if the pipe is busy
    let pipe = loop {
      match ClientOptions::new().open(&name) {
        Ok(pipe) => break pipe,
        Err(e) if e.raw_os_error() == Some(231) => {
          // ERROR_PIPE_BUSY (231) - All pipe instances are busy
          // Wait a bit and retry
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
  ///
  /// Returns a resource ID for the server.
  #[op2]
  #[smi]
  pub fn op_node_pipe_listen(
    state: &mut OpState,
    #[string] name: String,
  ) -> Result<ResourceId, PipeError> {
    let server = ServerOptions::new()
      .first_pipe_instance(true)
      .create(&name)?;

    let resource = NamedPipeServerResource::new(name, server);
    let rid = state.resource_table.add(resource);
    Ok(rid)
  }

  /// Wait for a client to connect to the named pipe server.
  ///
  /// Returns a resource ID for the connected client pipe.
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

    // Take the server out to wait for connection
    let mut server_cell =
      RcRef::map(&resource, |r| &r.server).borrow_mut().await;
    let server = server_cell.take().ok_or_else(|| {
      PipeError::ConnectionFailed("Server already accepting".to_string())
    })?;

    // Wait for a client to connect
    let connect_result = server.connect().try_or_cancel(cancel).await;

    match connect_result {
      Ok(Ok(())) => {
        // Connection successful - the server becomes the connection
        // We need to create a new server instance for future connections
        let new_server = ServerOptions::new().create(&name)?;
        *server_cell = Some(new_server);

        // Convert the connected server to a client-like resource
        // The NamedPipeServer after connect() can be used for read/write
        let client_resource = NamedPipeServerConnectionResource::new(server);
        let client_rid = state.borrow_mut().resource_table.add(client_resource);
        Ok(client_rid)
      }
      Ok(Err(e)) => {
        // Put the server back
        *server_cell = Some(server);
        Err(PipeError::Io(e))
      }
      Err(e) => {
        // Canceled - put the server back
        *server_cell = Some(server);
        Err(PipeError::Canceled(e))
      }
    }
  }

  /// Resource representing a connected Windows named pipe (from server side).
  pub struct NamedPipeServerConnectionResource {
    pipe: AsyncRefCell<NamedPipeServer>,
    cancel: CancelHandle,
  }

  impl Resource for NamedPipeServerConnectionResource {
    fn name(&self) -> std::borrow::Cow<str> {
      "namedPipeServerConnection".into()
    }

    fn close(self: Rc<Self>) {
      self.cancel.cancel();
    }

    deno_core::impl_readable_byob!();
    deno_core::impl_writable!();
  }

  impl NamedPipeServerConnectionResource {
    pub fn new(pipe: NamedPipeServer) -> Self {
      Self {
        pipe: AsyncRefCell::new(pipe),
        cancel: CancelHandle::new(),
      }
    }

    pub async fn read(
      self: Rc<Self>,
      data: &mut [u8],
    ) -> Result<usize, PipeError> {
      let mut pipe = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
      let cancel = RcRef::map(&self, |r| &r.cancel);
      Ok(pipe.read(data).try_or_cancel(cancel).await??)
    }

    pub async fn write(
      self: Rc<Self>,
      data: &[u8],
    ) -> Result<usize, PipeError> {
      let mut pipe = RcRef::map(&self, |r| &r.pipe).borrow_mut().await;
      let nwritten = pipe.write(data).await?;
      pipe.flush().await?;
      Ok(nwritten)
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

  #[op2(async)]
  #[smi]
  pub async fn op_node_pipe_accept(
    _state: Rc<RefCell<OpState>>,
    #[smi] _rid: ResourceId,
  ) -> Result<ResourceId, NotSupportedError> {
    Err(NotSupportedError)
  }
}
