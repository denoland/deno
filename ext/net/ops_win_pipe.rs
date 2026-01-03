// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::op2;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use tokio::net::windows::named_pipe;

use crate::ops::NetError;
use crate::win_pipe::NamedPipe;

#[op2(stack_trace)]
#[smi]
pub fn op_pipe_open(
  state: &mut OpState,
  #[string] path: String,
  #[smi] max_instances: Option<u32>,
  is_message_mode: bool,
  inbound: bool,
  outbound: bool,
  #[string] api_name: String,
) -> Result<ResourceId, NetError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();

  let path = permissions
    .check_open(
      Cow::Borrowed(Path::new(&path)),
      OpenAccessKind::ReadWriteNoFollow,
      Some(&api_name),
    )
    .map_err(NetError::Permission)?;

  let pipe_mode = if is_message_mode {
    named_pipe::PipeMode::Message
  } else {
    named_pipe::PipeMode::Byte
  };

  let mut opts = named_pipe::ServerOptions::new();
  opts
    .pipe_mode(pipe_mode)
    .access_inbound(inbound)
    .access_outbound(outbound);
  if let Some(max_instances) = max_instances {
    opts.max_instances(max_instances as usize);
  }
  let pipe = NamedPipe::new_server(AsRef::<Path>::as_ref(&path), &opts)?;
  let rid = state.resource_table.add(pipe);
  Ok(rid)
}

#[op2(async, stack_trace)]
pub async fn op_pipe_windows_wait(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(), NetError> {
  let pipe = state.borrow().resource_table.get::<NamedPipe>(rid)?;
  pipe.connect().await?;
  Ok(())
}

#[op2(fast)]
#[smi]
pub fn op_pipe_connect(
  state: &mut OpState,
  #[string] path: String,
  read: bool,
  write: bool,
  #[string] api_name: &str,
) -> Result<ResourceId, NetError> {
  let permissions = state.borrow_mut::<PermissionsContainer>();

  let checked_path = permissions
    .check_open(
      Cow::Borrowed(Path::new(&path)),
      OpenAccessKind::ReadWriteNoFollow,
      Some(api_name),
    )
    .map_err(NetError::Permission)?;

  // Check if this looks like a named pipe path
  // Windows named pipes must start with \\.\pipe\ or \\?\pipe\
  let is_named_pipe = path.starts_with("\\\\.\\pipe\\")
    || path.starts_with("\\\\?\\pipe\\")
    || path.starts_with("//./pipe/")
    || path.starts_with("//?/pipe/");

  if !is_named_pipe {
    // For non-pipe paths, check if the path exists as a file
    // If it does, return ENOTSOCK (not a socket)
    // If it doesn't exist, return ENOENT
    let path = Path::new(&path);
    if path.exists() {
      return Err(NetError::Io(std::io::Error::other(
        "ENOTSOCK: not a socket",
      )));
    } else {
      return Err(NetError::Io(std::io::Error::other(
        "ENOENT: no such file or directory",
      )));
    }
  }

  let mut opts = named_pipe::ClientOptions::new();
  opts.read(read).write(write);
  let pipe =
    NamedPipe::new_client(AsRef::<Path>::as_ref(&checked_path), &opts)?;
  let rid = state.resource_table.add(pipe);
  Ok(rid)
}
