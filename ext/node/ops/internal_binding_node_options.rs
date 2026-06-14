// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;

use crate::ExtNodeSys;

#[derive(Default)]
pub struct NodeOptionsState {
  exec_argv_snapshot: Option<Vec<String>>,
  options_map: Option<v8::Global<v8::Map>>,
  exec_argv_options_map: Option<v8::Global<v8::Map>>,
}

enum OptionValue {
  Bool(bool),
  String(String),
}

fn set_value(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: v8::Local<v8::Value>,
) {
  let key = v8::String::new(scope, name).unwrap();
  obj.set(scope, key.into(), value);
}

fn option_value_object<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  value: OptionValue,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  let value = match value {
    OptionValue::Bool(value) => v8::Boolean::new(scope, value).into(),
    OptionValue::String(value) => {
      v8::String::new(scope, &value).unwrap().into()
    }
  };
  set_value(scope, obj, "value", value);
  obj
}

fn set_option(
  scope: &mut v8::PinScope,
  map: v8::Local<v8::Map>,
  name: &str,
  value: OptionValue,
) {
  let key = v8::String::new(scope, name).unwrap();
  let value = option_value_object(scope, value);
  map.set(scope, key.into(), value.into()).unwrap();
}

fn split_node_options(input: &str) -> Vec<String> {
  let mut args = Vec::new();
  let mut current = String::new();
  let mut in_double = false;
  let mut in_single = false;

  for ch in input.chars() {
    if ch == '"' && !in_single {
      in_double = !in_double;
    } else if ch == '\'' && !in_double {
      in_single = !in_single;
    } else if ch.is_ascii_whitespace() && !in_double && !in_single {
      if !current.is_empty() {
        args.push(std::mem::take(&mut current));
      }
    } else {
      current.push(ch);
    }
  }
  if !current.is_empty() {
    args.push(current);
  }
  args
}

fn parse_option(
  scope: &mut v8::PinScope,
  options: v8::Local<v8::Map>,
  arg: &str,
) {
  if let Some(value) = arg.strip_prefix("--title=") {
    set_option(
      scope,
      options,
      "--title",
      OptionValue::String(value.to_string()),
    );
    return;
  }
  if let Some(value) = arg.strip_prefix("--tls-cipher-list=") {
    set_option(
      scope,
      options,
      "--tls-cipher-list",
      OptionValue::String(value.to_string()),
    );
    return;
  }
  match arg {
    "--no-warnings" => {
      set_option(scope, options, "--warnings", OptionValue::Bool(false))
    }
    "--pending-deprecation" => set_option(
      scope,
      options,
      "--pending-deprecation",
      OptionValue::Bool(true),
    ),
    "--expose-internals" | "--expose_internals" => set_option(
      scope,
      options,
      "--expose-internals",
      OptionValue::Bool(true),
    ),
    "--tls-min-v1.0" | "--tls-min-v1.1" | "--tls-min-v1.2"
    | "--tls-min-v1.3" | "--tls-max-v1.2" | "--tls-max-v1.3"
    | "--use-bundled-ca" | "--use-openssl-ca" | "--use-system-ca" => {
      set_option(scope, options, arg, OptionValue::Bool(true));
    }
    "--no-tls-min-v1.0" => {
      set_option(scope, options, "--tls-min-v1.0", OptionValue::Bool(false))
    }
    "--no-tls-min-v1.1" => {
      set_option(scope, options, "--tls-min-v1.1", OptionValue::Bool(false))
    }
    "--no-tls-min-v1.2" => {
      set_option(scope, options, "--tls-min-v1.2", OptionValue::Bool(false))
    }
    "--no-tls-min-v1.3" => {
      set_option(scope, options, "--tls-min-v1.3", OptionValue::Bool(false))
    }
    "--no-tls-max-v1.2" => {
      set_option(scope, options, "--tls-max-v1.2", OptionValue::Bool(false))
    }
    "--no-tls-max-v1.3" => {
      set_option(scope, options, "--tls-max-v1.3", OptionValue::Bool(false))
    }
    "--no-use-bundled-ca" => {
      set_option(scope, options, "--use-bundled-ca", OptionValue::Bool(false))
    }
    "--no-use-openssl-ca" => {
      set_option(scope, options, "--use-openssl-ca", OptionValue::Bool(false))
    }
    "--no-use-system-ca" => {
      set_option(scope, options, "--use-system-ca", OptionValue::Bool(false))
    }
    _ => {
      if let Some(value) = arg.strip_prefix("--dns-result-order=") {
        set_option(
          scope,
          options,
          "--dns-result-order",
          OptionValue::String(value.to_string()),
        );
      }
    }
  }
}

fn create_default_options<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Map> {
  let options = v8::Map::new(scope);
  set_option(scope, options, "--warnings", OptionValue::Bool(true));
  set_option(
    scope,
    options,
    "--pending-deprecation",
    OptionValue::Bool(false),
  );
  set_option(
    scope,
    options,
    "--expose-internals",
    OptionValue::Bool(false),
  );
  set_option(
    scope,
    options,
    "--title",
    OptionValue::String(String::new()),
  );
  options
}

fn process_object<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> Option<v8::Local<'s, v8::Object>> {
  let context = scope.get_current_context();
  let global = context.global(scope);
  let key = v8::String::new(scope, "process").unwrap();
  let process = global.get(scope, key.into())?;
  v8::Local::<v8::Object>::try_from(process).ok()
}

fn process_exec_argv(scope: &mut v8::PinScope, state: &OpState) -> Vec<String> {
  if let Some(snapshot) = &state.borrow::<NodeOptionsState>().exec_argv_snapshot
  {
    return snapshot.clone();
  }
  let Some(process) = process_object(scope) else {
    return Vec::new();
  };
  let key = v8::String::new(scope, "execArgv").unwrap();
  let Some(value) = process.get(scope, key.into()) else {
    return Vec::new();
  };
  let Ok(array) = v8::Local::<v8::Array>::try_from(value) else {
    return Vec::new();
  };
  let mut args = Vec::with_capacity(array.length() as usize);
  for index in 0..array.length() {
    let Some(value) = array.get_index(scope, index) else {
      continue;
    };
    let Some(value) = value.to_string(scope) else {
      continue;
    };
    args.push(value.to_rust_string_lossy(scope));
  }
  args
}

fn options_result<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  options: v8::Local<'s, v8::Map>,
) -> v8::Local<'s, v8::Object> {
  let result = v8::Object::new(scope);
  set_value(scope, result, "options", options.into());
  result
}

#[op2]
pub fn op_node_options_set_exec_argv(
  state: &mut OpState,
  #[serde] exec_argv: Vec<String>,
) {
  let state = state.borrow_mut::<NodeOptionsState>();
  state.exec_argv_snapshot = Some(exec_argv);
  state.options_map = None;
  state.exec_argv_options_map = None;
}

#[op2]
pub fn op_node_options_get_options<'s, TSys: ExtNodeSys + 'static>(
  state: &mut OpState,
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let cached = state.borrow::<NodeOptionsState>().options_map.clone();
  if let Some(cached) = cached {
    let options = v8::Local::new(scope, &cached);
    return options_result(scope, options);
  }

  let options = create_default_options(scope);
  let node_options = {
    if let Some(sys) = state.try_borrow::<TSys>() {
      sys.env_var("NODE_OPTIONS").ok()
    } else {
      std::env::var("NODE_OPTIONS").ok()
    }
  };
  if let Some(node_options) = node_options {
    for arg in split_node_options(&node_options) {
      parse_option(scope, options, &arg);
    }
  }
  for arg in process_exec_argv(scope, state) {
    parse_option(scope, options, &arg);
  }
  state.borrow_mut::<NodeOptionsState>().options_map =
    Some(v8::Global::new(scope, options));
  options_result(scope, options)
}

#[op2]
pub fn op_node_options_get_exec_argv_options<'s>(
  state: &mut OpState,
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let cached = state
    .borrow::<NodeOptionsState>()
    .exec_argv_options_map
    .clone();
  if let Some(cached) = cached {
    let options = v8::Local::new(scope, &cached);
    return options_result(scope, options);
  }

  let options = v8::Map::new(scope);
  for arg in process_exec_argv(scope, state) {
    parse_option(scope, options, &arg);
  }
  state.borrow_mut::<NodeOptionsState>().exec_argv_options_map =
    Some(v8::Global::new(scope, options));
  options_result(scope, options)
}
