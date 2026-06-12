// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;

use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::ExternalReference;
use deno_core::v8::MapFnTo;
use deno_dotenv::parse_env_content_hook;
use deno_error::JsErrorBox;
use node_resolver::InNpmPackageChecker;
use node_resolver::NpmPackageFolderResolver;

use crate::ExtNodeSys;
use crate::NodeResolverRc;

#[repr(u32)]
enum HandleType {
  #[allow(dead_code, reason = "variant kept for repr(u32) mapping")]
  Tcp = 0,
  Tty,
  #[allow(dead_code, reason = "variant kept for repr(u32) mapping")]
  Udp,
  File,
  Pipe,
  Unknown,
}

const HANDLE_TYPES: [&str; 6] =
  ["TCP", "TTY", "UDP", "FILE", "PIPE", "UNKNOWN"];

#[op2(fast)]
pub fn op_node_guess_handle_type(_state: &mut OpState, fd: u32) -> u32 {
  guess_handle_type(fd as i32) as u32
}

#[cfg(unix)]
fn guess_handle_type(fd: i32) -> HandleType {
  use deno_core::uv_compat;
  match uv_compat::uv_guess_handle(fd) {
    uv_compat::uv_handle_type::UV_TCP => HandleType::Tcp,
    uv_compat::uv_handle_type::UV_TTY => HandleType::Tty,
    uv_compat::uv_handle_type::UV_UDP => HandleType::Unknown,
    uv_compat::uv_handle_type::UV_FILE => HandleType::File,
    uv_compat::uv_handle_type::UV_NAMED_PIPE => HandleType::Pipe,
    _ => HandleType::Unknown,
  }
}

#[cfg(windows)]
fn guess_handle_type(fd: i32) -> HandleType {
  use windows_sys::Win32::Storage::FileSystem::FILE_TYPE_CHAR;
  use windows_sys::Win32::Storage::FileSystem::FILE_TYPE_DISK;
  use windows_sys::Win32::Storage::FileSystem::FILE_TYPE_PIPE;
  use windows_sys::Win32::Storage::FileSystem::GetFileType;
  use windows_sys::Win32::System::Console::GetConsoleMode;

  if fd < 0 {
    return HandleType::Unknown;
  }
  // SAFETY: get_osfhandle converts a CRT fd to an OS handle.
  // Returns -1 (INVALID_HANDLE_VALUE) for invalid fds.
  let handle = unsafe { libc::get_osfhandle(fd) };
  if handle == -1 {
    return HandleType::Unknown;
  }
  let handle = handle as windows_sys::Win32::Foundation::HANDLE;
  // SAFETY: handle is a valid OS handle from get_osfhandle.
  match unsafe { GetFileType(handle) } {
    FILE_TYPE_DISK => HandleType::File,
    FILE_TYPE_CHAR => {
      let mut mode = 0;
      // SAFETY: handle is valid, mode is a valid pointer.
      if unsafe { GetConsoleMode(handle, &mut mode) } == 1 {
        HandleType::Tty
      } else {
        HandleType::File
      }
    }
    FILE_TYPE_PIPE => HandleType::Pipe,
    _ => HandleType::Unknown,
  }
}

#[op2(fast)]
pub fn op_node_view_has_buffer(buffer: v8::Local<v8::ArrayBufferView>) -> bool {
  buffer.has_buffer()
}

/// Checks if the current call site is from a dependency package.
#[op2(fast)]
pub fn op_node_call_is_from_dependency<
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  scope: &mut v8::PinScope<'_, '_>,
) -> bool {
  // non internal call site should appear in < 20 frames
  let Some(stack_trace) = v8::StackTrace::current_stack_trace(scope, 20) else {
    return false;
  };
  let mut only_internal = true;
  for i in 0..stack_trace.get_frame_count() {
    let Some(frame) = stack_trace.get_frame(scope, i) else {
      continue;
    };
    if !frame.is_user_javascript() {
      continue;
    }
    let Some(script) = frame.get_script_name(scope) else {
      continue;
    };
    let mut name_buf: [std::mem::MaybeUninit<u8>; 1024] =
      [std::mem::MaybeUninit::uninit(); 1024];
    let name = script.to_rust_cow_lossy(scope, &mut name_buf);

    if name.starts_with("node:") || name.starts_with("ext:") {
      continue;
    } else {
      only_internal = false;
    }

    if name.starts_with("https:")
      || name.contains("/node_modules/")
      || name.contains(r"\node_modules\")
    {
      return true;
    }

    let Ok(specifier) = url::Url::parse(&name) else {
      continue;
    };
    if only_internal {
      return true;
    }
    return state.borrow::<NodeResolverRc<
        TInNpmPackageChecker,
        TNpmPackageFolderResolver,
        TSys,
      >>().in_npm_package(&specifier);
  }
  only_internal
}

#[op2(fast)]
pub fn op_node_in_npm_package<
  TInNpmPackageChecker: InNpmPackageChecker + 'static,
  TNpmPackageFolderResolver: NpmPackageFolderResolver + 'static,
  TSys: ExtNodeSys + 'static,
>(
  state: &mut OpState,
  #[string] path: &str,
) -> bool {
  let specifier = if deno_path_util::specifier_has_uri_scheme(path) {
    match url::Url::parse(path) {
      Ok(url) => url,
      Err(_) => return false,
    }
  } else {
    match deno_path_util::url_from_file_path(Path::new(path)) {
      Ok(url) => url,
      Err(_) => return false,
    }
  };

  state.borrow::<NodeResolverRc<
    TInNpmPackageChecker,
    TNpmPackageFolderResolver,
    TSys,
  >>().in_npm_package(&specifier)
}

#[op2]
pub fn op_node_get_own_non_index_properties<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  #[smi] filter: u32,
) -> Result<v8::Local<'s, v8::Value>, JsErrorBox> {
  let mut property_filter = v8::PropertyFilter::ALL_PROPERTIES;
  if filter & 1 << 0 != 0 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_WRITABLE;
  }
  if filter & 1 << 1 != 0 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_ENUMERABLE;
  }
  if filter & 1 << 2 != 0 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_CONFIGURABLE;
  }
  if filter & 1 << 3 != 0 {
    property_filter = property_filter | v8::PropertyFilter::SKIP_STRINGS;
  }
  if filter & 1 << 4 != 0 {
    property_filter = property_filter | v8::PropertyFilter::SKIP_SYMBOLS;
  }

  v8::tc_scope!(let tc_scope, scope);

  let result = obj.get_property_names(
    tc_scope,
    v8::GetPropertyNamesArgs {
      index_filter: v8::IndexFilter::SkipIndices,
      property_filter,
      key_conversion: v8::KeyConversionMode::NoNumbers,
      mode: v8::KeyCollectionMode::OwnOnly,
    },
  );

  match result {
    Some(names) => Ok(names.into()),
    None => {
      if tc_scope.has_caught() || tc_scope.has_terminated() {
        tc_scope.rethrow();
        // Dummy value, this result will be discarded because an error was thrown.
        let v = v8::undefined(tc_scope);
        Ok(v.into())
      } else {
        Err(JsErrorBox::type_error(
          "Failed to get own non-index properties",
        ))
      }
    }
  }
}

#[op2]
pub fn op_node_parse_env<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[string] content: &str,
) -> v8::Local<'a, v8::Object> {
  parse_env(scope, content)
}

fn parse_env<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  content: &str,
) -> v8::Local<'a, v8::Object> {
  let env_obj = v8::Object::new(scope);
  parse_env_content_hook(content, &mut |key, value| {
    let key = v8::String::new(scope, key).unwrap();
    let value = v8::String::new(scope, value).unwrap();
    env_obj.set(scope, key.into(), value.into());
  });
  env_obj
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

fn set_i32(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: i32,
) {
  let value = v8::Integer::new(scope, value);
  set_value(scope, obj, name, value.into());
}

fn set_function(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  export_name: &str,
  function: v8::Local<v8::Function>,
) {
  let name = v8::String::new(scope, export_name).unwrap();
  function.set_name(name);
  set_value(scope, obj, export_name, function.into());
}

fn throw_type_error(scope: &mut v8::PinScope, message: &str) {
  let message = v8::String::new(scope, message).unwrap();
  let exception = v8::Exception::type_error(scope, message);
  scope.throw_exception(exception);
}

fn guess_handle_type_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let fd = args.get(0).integer_value(scope).unwrap_or(-1) as i32;
  let handle_type = guess_handle_type(fd) as usize;
  let value = v8::String::new(
    scope,
    HANDLE_TYPES.get(handle_type).copied().unwrap_or("UNKNOWN"),
  )
  .unwrap();
  rv.set(value.into());
}

fn is_array_index_value(
  scope: &mut v8::PinScope,
  value: v8::Local<v8::Value>,
) -> bool {
  if value.is_number() {
    let Some(number) = value.number_value(scope) else {
      return false;
    };
    return number >= 0.0
      && number.fract() == 0.0
      && number as i32 as f64 == number;
  }
  if value.is_string() {
    let value = value.to_string(scope).unwrap().to_rust_string_lossy(scope);
    if value.is_empty() {
      return false;
    }
    if value.len() > 1 && value.as_bytes()[0] == b'0' {
      return false;
    }
    return value.bytes().all(|ch| ch.is_ascii_digit());
  }
  false
}

fn is_array_index_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  rv.set_bool(is_array_index_value(scope, args.get(0)));
}

fn get_own_non_index_properties_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let value = args.get(0);
  if !value.is_object() {
    throw_type_error(scope, "obj must be an object");
    return;
  }
  let obj = v8::Local::<v8::Object>::try_from(value).unwrap();
  let filter = args.get(1).uint32_value(scope).unwrap_or(0);
  let mut property_filter = v8::PropertyFilter::ALL_PROPERTIES;
  if filter & 1 << 0 != 0 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_WRITABLE;
  }
  if filter & 1 << 1 != 0 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_ENUMERABLE;
  }
  if filter & 1 << 2 != 0 {
    property_filter = property_filter | v8::PropertyFilter::ONLY_CONFIGURABLE;
  }
  if filter & 1 << 3 != 0 {
    property_filter = property_filter | v8::PropertyFilter::SKIP_STRINGS;
  }
  if filter & 1 << 4 != 0 {
    property_filter = property_filter | v8::PropertyFilter::SKIP_SYMBOLS;
  }

  v8::tc_scope!(let tc_scope, scope);
  let result = obj.get_property_names(
    tc_scope,
    v8::GetPropertyNamesArgs {
      index_filter: v8::IndexFilter::SkipIndices,
      property_filter,
      key_conversion: v8::KeyConversionMode::NoNumbers,
      mode: v8::KeyCollectionMode::OwnOnly,
    },
  );
  match result {
    Some(names) => rv.set(names.into()),
    None => {
      if tc_scope.has_caught() || tc_scope.has_terminated() {
        tc_scope.rethrow();
      } else {
        throw_type_error(tc_scope, "Failed to get own non-index properties");
      }
    }
  }
}

fn array_buffer_view_has_buffer_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let value = args.get(0);
  if !value.is_array_buffer_view() {
    throw_type_error(scope, "view must be an ArrayBufferView");
    return;
  }
  let view = v8::Local::<v8::ArrayBufferView>::try_from(value).unwrap();
  rv.set_bool(view.has_buffer());
}

fn parse_env_callback(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  let Some(content) = args.get(0).to_string(scope) else {
    throw_type_error(scope, "env must be a string");
    return;
  };
  let content = content.to_rust_string_lossy(scope);
  let env = parse_env(scope, &content);
  rv.set(env.into());
}

pub(crate) fn external_references() -> [ExternalReference; 5] {
  [
    GUESS_HANDLE_TYPE_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    IS_ARRAY_INDEX_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    GET_OWN_NON_INDEX_PROPERTIES_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    ARRAY_BUFFER_VIEW_HAS_BUFFER_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
    PARSE_ENV_CALLBACK.with(|callback| ExternalReference {
      function: *callback,
    }),
  ]
}

thread_local! {
  static GUESS_HANDLE_TYPE_CALLBACK: v8::FunctionCallback = guess_handle_type_callback.map_fn_to();
  static IS_ARRAY_INDEX_CALLBACK: v8::FunctionCallback = is_array_index_callback.map_fn_to();
  static GET_OWN_NON_INDEX_PROPERTIES_CALLBACK: v8::FunctionCallback = get_own_non_index_properties_callback.map_fn_to();
  static ARRAY_BUFFER_VIEW_HAS_BUFFER_CALLBACK: v8::FunctionCallback = array_buffer_view_has_buffer_callback.map_fn_to();
  static PARSE_ENV_CALLBACK: v8::FunctionCallback = parse_env_callback.map_fn_to();
}

fn function_from_callback<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  callback: v8::FunctionCallback,
) -> v8::Local<'s, v8::Function> {
  v8::FunctionTemplate::new_raw(scope, callback)
    .get_function(scope)
    .unwrap()
}

#[op2]
pub fn op_node_internal_binding_util<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);

  let guess_handle_type = GUESS_HANDLE_TYPE_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "guessHandleType", guess_handle_type);
  let is_array_index = IS_ARRAY_INDEX_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "isArrayIndex", is_array_index);
  let get_own_non_index_properties = GET_OWN_NON_INDEX_PROPERTIES_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(
    scope,
    obj,
    "getOwnNonIndexProperties",
    get_own_non_index_properties,
  );
  let array_buffer_view_has_buffer = ARRAY_BUFFER_VIEW_HAS_BUFFER_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(
    scope,
    obj,
    "arrayBufferViewHasBuffer",
    array_buffer_view_has_buffer,
  );
  let parse_env = PARSE_ENV_CALLBACK
    .with(|callback| function_from_callback(scope, *callback));
  set_function(scope, obj, "parseEnv", parse_env);

  for (name, value) in [
    ("ALL_PROPERTIES", 0),
    ("ONLY_WRITABLE", 1),
    ("ONLY_ENUMERABLE", 2),
    ("ONLY_CONFIGURABLE", 4),
    ("ONLY_ENUM_WRITABLE", 6),
    ("SKIP_STRINGS", 8),
    ("SKIP_SYMBOLS", 16),
  ] {
    set_i32(scope, obj, name, value);
  }

  let symbol_name =
    v8::String::new(scope, "nodejs.worker_threads.untransferable").unwrap();
  let untransferable_symbol = v8::Symbol::for_key(scope, symbol_name);
  set_value(
    scope,
    obj,
    "untransferableSymbol",
    untransferable_symbol.into(),
  );

  let default_obj = v8::Object::new(scope);
  for name in [
    "guessHandleType",
    "isArrayIndex",
    "getOwnNonIndexProperties",
    "arrayBufferViewHasBuffer",
    "parseEnv",
    "untransferableSymbol",
  ] {
    let key = v8::String::new(scope, name).unwrap();
    let value = obj.get(scope, key.into()).unwrap();
    set_value(scope, default_obj, name, value);
  }
  set_value(scope, obj, "default", default_obj.into());
  obj
}
