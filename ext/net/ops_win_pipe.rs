// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::op2;
use deno_core::OpState;
use deno_core::ResourceId;
use serde::Deserialize;
use tokio::net::windows::named_pipe;

use crate::ops::NetError;
use crate::win_pipe::NamedPipe;
use crate::NetPermissions;

fn is_true() -> bool {
  true
}

#[derive(Deserialize, Debug)]
pub enum PipeMode {
  #[serde(rename = "message")]
  Message,
  #[serde(rename = "byte")]
  Byte,
}

impl From<PipeMode> for named_pipe::PipeMode {
  fn from(value: PipeMode) -> Self {
    match value {
      PipeMode::Message => named_pipe::PipeMode::Message,
      PipeMode::Byte => named_pipe::PipeMode::Byte,
    }
  }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ListenArgs {
  path: String,
  max_instances: Option<usize>,
  pipe_mode: PipeMode,
  #[serde(default = "is_true")]
  inbound: bool,
  #[serde(default = "is_true")]
  outbound: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ConnectArgs {
  path: String,
  #[serde(default = "is_true")]
  read: bool,
  #[serde(default = "is_true")]
  write: bool,
}

#[op2(stack_trace)]
#[serde]
pub fn op_pipe_open<NP>(
  state: &mut OpState,
  #[serde] args: ListenArgs,
  #[string] api_name: String,
) -> Result<ResourceId, NetError>
where
  NP: NetPermissions + 'static,
{
  let permissions = state.borrow_mut::<NP>();

  let api_call_expr = format!("{}()", api_name);
  let path = {
    let path = if args.inbound {
      permissions
        .check_read(&args.path, &api_call_expr)
        .map_err(NetError::Permission)?
    } else {
      PathBuf::from(args.path)
    };

    permissions
      .check_write_path(Cow::Owned(path), &api_call_expr)
      .map_err(NetError::Permission)?
  };

  let mut opts = named_pipe::ServerOptions::new();
  opts
    .pipe_mode(args.pipe_mode.into())
    .access_inbound(args.inbound)
    .access_outbound(args.outbound);
  if args.max_instances.is_some() {
    opts.max_instances(args.max_instances.unwrap());
  }
  let pipe = NamedPipe::new_server(path.as_ref(), &opts)?;
  let rid = state.resource_table.add(pipe);
  Ok(rid)
}

#[op2(async, stack_trace)]
pub async fn op_pipe_windows_wait(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(), NetError> {
  let pipe = state.borrow().resource_table.get::<NamedPipe>(rid)?;
  drop(state);
  pipe.connect().await?;
  Ok(())
}

#[op2(stack_trace)]
#[smi]
pub fn op_pipe_connect<NP>(
  state: &mut OpState,
  #[serde] args: ConnectArgs,
  #[string] api_name: &str,
) -> Result<ResourceId, NetError>
where
  NP: NetPermissions + 'static,
{
  let permissions = state.borrow_mut::<NP>();

  let api_call_expr = format!("{}()", api_name);
  let path = {
    let path = if args.read {
      permissions
        .check_read(&args.path, &api_call_expr)
        .map_err(NetError::Permission)?
    } else {
      PathBuf::from(args.path)
    };

    if args.write {
      permissions
        .check_write_path(Cow::Owned(path), &api_call_expr)
        .map_err(NetError::Permission)?
    } else {
      Cow::Owned(path)
    }
  };

  let mut opts = named_pipe::ClientOptions::new();
  opts.read(args.read).write(args.write);
  let pipe = NamedPipe::new_client(path.as_ref(), &opts)?;
  let rid = state.resource_table.add(pipe);
  Ok(rid)
}
