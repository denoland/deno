// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{wrap_json_op, Deserialize, JsonOp};
use crate::ansi;
use crate::fs as deno_fs;
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use crate::version;
use atty;
use deno::*;
use log;
use std::collections::HashMap;
use std::env;
use url::Url;

// Start

pub struct OpStart;

impl DenoOpDispatcher for OpStart {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |_args, _zero_copy| {
        let state = state.clone();
        Ok(JsonOp::Sync(json!({
          "cwd": deno_fs::normalize_path(&env::current_dir().unwrap()),
          "pid": std::process::id(),
          "argv": state.argv,
          "mainModule": state.main_module().map(|x| x.as_str().to_string()),
          "debugFlag": state
            .flags
            .log_level
            .map_or(false, |l| l == log::Level::Debug),
          "versionFlag": state.flags.version,
          "v8Version": version::v8(),
          "denoVersion": version::DENO,
          "noColor": !ansi::use_color(),
          "xevalDelim": state.flags.xeval_delim.clone(),
        })))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "start";
}

// Home Dir

pub struct OpHomeDir;

impl DenoOpDispatcher for OpHomeDir {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |_args, _zero_copy| {
        state.check_env()?;
        let path = dirs::home_dir()
          .unwrap_or_default()
          .into_os_string()
          .into_string()
          .unwrap_or_default();
        Ok(JsonOp::Sync(json!(path)))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "homeDir";
}

// Exec Path

pub struct OpExecPath;

impl DenoOpDispatcher for OpExecPath {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |_args, _zero_copy| {
        state.check_env()?;
        let current_exe = env::current_exe().unwrap();
        // Now apply URL parser to current exe to get fully resolved path, otherwise
        // we might get `./` and `../` bits in `exec_path`
        let exe_url = Url::from_file_path(current_exe).unwrap();
        let path = exe_url.to_file_path().unwrap();
        Ok(JsonOp::Sync(json!(path)))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "execPath";
}

// Set Env

pub struct OpSetEnv;

#[derive(Deserialize)]
struct SetEnvArgs {
  key: String,
  value: String,
}

impl DenoOpDispatcher for OpSetEnv {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: SetEnvArgs = serde_json::from_value(args)?;
        state.check_env()?;
        env::set_var(args.key, args.value);
        Ok(JsonOp::Sync(json!({})))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "setEnv";
}

// Env

pub struct OpEnv;

impl DenoOpDispatcher for OpEnv {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |_args, _zero_copy| {
        state.check_env()?;
        let v = env::vars().collect::<HashMap<String, String>>();
        Ok(JsonOp::Sync(json!(v)))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "env";
}

// Exit

pub struct OpExit;

#[derive(Deserialize)]
struct ExitArgs {
  code: i32,
}

impl DenoOpDispatcher for OpExit {
  fn dispatch(
    &self,
    _state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: ExitArgs = serde_json::from_value(args)?;
        std::process::exit(args.code)
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "exit";
}

// Is Tty

pub struct OpIsTty;

impl DenoOpDispatcher for OpIsTty {
  fn dispatch(
    &self,
    _state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |_args, _zero_copy| {
        Ok(JsonOp::Sync(json!({
          "stdin": atty::is(atty::Stream::Stdin),
          "stdout": atty::is(atty::Stream::Stdout),
          "stderr": atty::is(atty::Stream::Stderr),
        })))
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "isTty";
}
