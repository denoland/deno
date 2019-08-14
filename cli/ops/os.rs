// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::ansi;
use crate::fs as deno_fs;
use crate::msg;
use crate::msg_util;
use crate::ops::empty_buf;
use crate::ops::ok_buf;
use crate::ops::serialize_response;
use crate::ops::CliOpResult;
use crate::state::ThreadSafeState;
use crate::version;
use atty;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use log;
use url::Url;

pub fn op_start(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let mut builder = FlatBufferBuilder::new();

  let state = state;
  let argv = state.argv.iter().map(String::as_str).collect::<Vec<_>>();
  let argv_off = builder.create_vector_of_strings(argv.as_slice());

  let cwd_path = std::env::current_dir().unwrap();
  let cwd_off =
    builder.create_string(deno_fs::normalize_path(cwd_path.as_ref()).as_ref());

  let v8_version = version::v8();
  let v8_version_off = builder.create_string(v8_version);

  let deno_version = version::DENO;
  let deno_version_off = builder.create_string(deno_version);

  let main_module = state
    .main_module()
    .map(|m| builder.create_string(&m.to_string()));

  let xeval_delim = state
    .flags
    .xeval_delim
    .clone()
    .map(|m| builder.create_string(&m));

  let debug_flag = state
    .flags
    .log_level
    .map_or(false, |l| l == log::Level::Debug);

  let inner = msg::StartRes::create(
    &mut builder,
    &msg::StartResArgs {
      cwd: Some(cwd_off),
      pid: std::process::id(),
      argv: Some(argv_off),
      main_module,
      debug_flag,
      version_flag: state.flags.version,
      v8_version: Some(v8_version_off),
      deno_version: Some(deno_version_off),
      no_color: !ansi::use_color(),
      xeval_delim,
      ..Default::default()
    },
  );

  ok_buf(serialize_response(
    base.cmd_id(),
    &mut builder,
    msg::BaseArgs {
      inner_type: msg::Any::StartRes,
      inner: Some(inner.as_union_value()),
      ..Default::default()
    },
  ))
}

pub fn op_home_dir(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();

  state.check_env()?;

  let builder = &mut FlatBufferBuilder::new();
  let path = dirs::home_dir()
    .unwrap_or_default()
    .into_os_string()
    .into_string()
    .unwrap_or_default();
  let path = Some(builder.create_string(&path));
  let inner = msg::HomeDirRes::create(builder, &msg::HomeDirResArgs { path });

  ok_buf(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::HomeDirRes,
      ..Default::default()
    },
  ))
}

pub fn op_exec_path(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();

  state.check_env()?;

  let builder = &mut FlatBufferBuilder::new();
  let current_exe = std::env::current_exe().unwrap();
  // Now apply URL parser to current exe to get fully resolved path, otherwise we might get
  // `./` and `../` bits in `exec_path`
  let exe_url = Url::from_file_path(current_exe).unwrap();
  let path = exe_url.to_file_path().unwrap().to_str().unwrap().to_owned();
  let path = Some(builder.create_string(&path));
  let inner = msg::ExecPathRes::create(builder, &msg::ExecPathResArgs { path });

  ok_buf(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::ExecPathRes,
      ..Default::default()
    },
  ))
}

pub fn op_set_env(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_set_env().unwrap();
  let key = inner.key().unwrap();
  let value = inner.value().unwrap();
  state.check_env()?;
  std::env::set_var(key, value);
  ok_buf(empty_buf())
}

pub fn op_env(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();

  state.check_env()?;

  let builder = &mut FlatBufferBuilder::new();
  let vars: Vec<_> = std::env::vars()
    .map(|(key, value)| msg_util::serialize_key_value(builder, &key, &value))
    .collect();
  let tables = builder.create_vector(&vars);
  let inner = msg::EnvironRes::create(
    builder,
    &msg::EnvironResArgs { map: Some(tables) },
  );
  let response_buf = serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::EnvironRes,
      ..Default::default()
    },
  );
  ok_buf(response_buf)
}

pub fn op_exit(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  _data: Option<PinnedBuf>,
) -> CliOpResult {
  let inner = base.inner_as_exit().unwrap();
  std::process::exit(inner.code())
}

pub fn op_is_tty(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  _data: Option<PinnedBuf>,
) -> CliOpResult {
  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::IsTTYRes::create(
    builder,
    &msg::IsTTYResArgs {
      stdin: atty::is(atty::Stream::Stdin),
      stdout: atty::is(atty::Stream::Stdout),
      stderr: atty::is(atty::Stream::Stderr),
    },
  );
  ok_buf(serialize_response(
    base.cmd_id(),
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::IsTTYRes,
      ..Default::default()
    },
  ))
}
