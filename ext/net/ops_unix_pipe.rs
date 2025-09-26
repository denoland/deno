// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::io;
use std::path::Path;

use deno_core::op2;
use deno_core::OpState;
use deno_core::ResourceId;
use serde::Deserialize;

use crate::ops::NetError;
use crate::unix_pipe::NamedPipe;
use crate::NetPermissions;

fn is_false() -> bool {
  false
}

#[derive(Deserialize, Debug)]
struct ListenArgs {
  path: String,
  mode: Option<u32>,
  #[serde(default = "is_false")]
  create: bool,
}

pub fn create_pipe<'a, NP>(
  permissions: &'a mut NP,
  path: &str,
  mode: nix::sys::stat::Mode,
  api_name: &str,
) -> Result<Cow<'a, Path>, NetError>
where
  NP: NetPermissions + 'static,
{
  let path = {
    let path = permissions
      .check_read(path, api_name)
      .map_err(NetError::Permission)?;

    permissions
      .check_write_path(Cow::Owned(path), api_name)
      .map_err(NetError::Permission)?
  };

  nix::unistd::mkfifo(path.as_ref(), mode).map_err(io::Error::from)?;

  Ok(path)
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
  let path = if args.create {
    create_pipe(
      permissions,
      &args.path,
      args
        .mode
        .map(|mode| nix::sys::stat::Mode::from_bits(mode as _))
        .unwrap_or(Some(nix::sys::stat::Mode::S_IRWXU))
        .ok_or(NetError::Io(io::ErrorKind::InvalidInput.into()))?,
      &api_call_expr,
    )?
  } else {
    let path = permissions
      .check_read(&args.path, &api_call_expr)
      .map_err(NetError::Permission)?;

    permissions
      .check_write_path(Cow::Owned(path), &api_call_expr)
      .map_err(NetError::Permission)?
  };

  let pipe = NamedPipe::new_receiver(path)?;
  let rid = state.resource_table.add(pipe);
  Ok(rid)
}

#[op2(fast, stack_trace)]
#[smi]
pub fn op_pipe_connect<NP>(
  state: &mut OpState,
  #[string] path: String,
  #[string] api_name: &str,
) -> Result<ResourceId, NetError>
where
  NP: NetPermissions + 'static,
{
  let permissions = state.borrow_mut::<NP>();

  let api_call_expr = format!("{}()", api_name);
  let path = {
    let path = permissions
      .check_read(&path, &api_call_expr)
      .map_err(NetError::Permission)?;

    permissions
      .check_write_path(Cow::Owned(path), &api_call_expr)
      .map_err(NetError::Permission)?
  };

  let pipe = NamedPipe::new_sender(path)?;
  let rid = state.resource_table.add(pipe);
  Ok(rid)
}
