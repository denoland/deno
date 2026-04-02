// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;

use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
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
  use winapi::um::consoleapi::GetConsoleMode;
  use winapi::um::fileapi::GetFileType;
  use winapi::um::winbase::FILE_TYPE_CHAR;
  use winapi::um::winbase::FILE_TYPE_DISK;
  use winapi::um::winbase::FILE_TYPE_PIPE;

  if fd < 0 {
    return HandleType::Unknown;
  }
  // SAFETY: get_osfhandle converts a CRT fd to an OS handle.
  // Returns -1 (INVALID_HANDLE_VALUE) for invalid fds.
  let handle = unsafe { libc::get_osfhandle(fd) };
  if handle == -1 {
    return HandleType::Unknown;
  }
  let handle = handle as winapi::shared::ntdef::HANDLE;
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
  let env_obj = v8::Object::new(scope);
  parse_env_content_hook(content, &mut |key, value| {
    let key = v8::String::new(scope, key).unwrap();
    let value = v8::String::new(scope, value).unwrap();
    env_obj.set(scope, key.into(), value.into());
  });
  env_obj
}
