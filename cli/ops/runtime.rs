// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::metrics::Metrics;
use crate::permissions::Permissions;
use crate::version;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use std::env;

type ApplySourceMaps = bool;

pub fn init(
  rt: &mut deno_core::JsRuntime,
  main_module: ModuleSpecifier,
  apply_source_maps: bool,
) {
  {
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put::<ModuleSpecifier>(main_module);
    state.put::<ApplySourceMaps>(apply_source_maps);
  }
  super::reg_json_sync(rt, "op_start", op_start);
  super::reg_json_sync(rt, "op_main_module", op_main_module);
  super::reg_json_sync(rt, "op_metrics", op_metrics);
}

fn op_start(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let apply_source_maps = *state.borrow::<ApplySourceMaps>();
  let gs = &super::program_state(state);

  Ok(json!({
    "args": gs.flags.argv.clone(),
    "applySourceMaps": apply_source_maps,
    "debugFlag": gs.flags.log_level.map_or(false, |l| l == log::Level::Debug),
    "denoVersion": *version::DENO,
    "noColor": !colors::use_color(),
    "pid": std::process::id(),
    "ppid": ppid(),
    "target": env!("TARGET"),
    "tsVersion": version::TYPESCRIPT,
    "unstableFlag": gs.flags.unstable,
    "v8Version": version::v8(),
    "versionFlag": gs.flags.version,
  }))
}

fn op_main_module(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let main = state.borrow::<ModuleSpecifier>().to_string();
  let main_url = ModuleSpecifier::resolve_url_or_path(&main)?;
  if main_url.as_url().scheme() == "file" {
    let main_path = std::env::current_dir().unwrap().join(main_url.to_string());
    state
      .borrow::<Permissions>()
      .check_read_blind(&main_path, "main_module")?;
  }
  Ok(json!(&main))
}

fn op_metrics(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let m = state.borrow::<Metrics>();

  Ok(json!({
    "opsDispatched": m.ops_dispatched,
    "opsDispatchedSync": m.ops_dispatched_sync,
    "opsDispatchedAsync": m.ops_dispatched_async,
    "opsDispatchedAsyncUnref": m.ops_dispatched_async_unref,
    "opsCompleted": m.ops_completed,
    "opsCompletedSync": m.ops_completed_sync,
    "opsCompletedAsync": m.ops_completed_async,
    "opsCompletedAsyncUnref": m.ops_completed_async_unref,
    "bytesSentControl": m.bytes_sent_control,
    "bytesSentData": m.bytes_sent_data,
    "bytesReceived": m.bytes_received
  }))
}

fn ppid() -> Value {
  #[cfg(windows)]
  {
    // Adopted from rustup:
    // https://github.com/rust-lang/rustup/blob/1.21.1/src/cli/self_update.rs#L1036
    // Copyright Diggory Blake, the Mozilla Corporation, and rustup contributors.
    // Licensed under either of
    // - Apache License, Version 2.0
    // - MIT license
    use std::mem;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::processthreadsapi::GetCurrentProcessId;
    use winapi::um::tlhelp32::{
      CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32,
      TH32CS_SNAPPROCESS,
    };
    unsafe {
      // Take a snapshot of system processes, one of which is ours
      // and contains our parent's pid
      let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
      if snapshot == INVALID_HANDLE_VALUE {
        return serde_json::to_value(-1).unwrap();
      }

      let mut entry: PROCESSENTRY32 = mem::zeroed();
      entry.dwSize = mem::size_of::<PROCESSENTRY32>() as DWORD;

      // Iterate over system processes looking for ours
      let success = Process32First(snapshot, &mut entry);
      if success == 0 {
        CloseHandle(snapshot);
        return serde_json::to_value(-1).unwrap();
      }

      let this_pid = GetCurrentProcessId();
      while entry.th32ProcessID != this_pid {
        let success = Process32Next(snapshot, &mut entry);
        if success == 0 {
          CloseHandle(snapshot);
          return serde_json::to_value(-1).unwrap();
        }
      }
      CloseHandle(snapshot);

      // FIXME: Using the process ID exposes a race condition
      // wherein the parent process already exited and the OS
      // reassigned its ID.
      let parent_id = entry.th32ParentProcessID;
      serde_json::to_value(parent_id).unwrap()
    }
  }
  #[cfg(not(windows))]
  {
    use std::os::unix::process::parent_id;
    serde_json::to_value(parent_id()).unwrap()
  }
}
