// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;

use deno_core::OpState;
use deno_core::ResourceHandle;
use deno_core::ResourceHandleFd;
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
  #[allow(dead_code)]
  Tcp = 0,
  Tty,
  #[allow(dead_code)]
  Udp,
  File,
  Pipe,
  Unknown,
}

/// Check if a raw file descriptor is a TTY.
/// This is used by Node.js `tty.isatty(fd)`.
#[op2(fast)]
pub fn op_node_is_tty(fd: i32) -> bool {
  if fd < 0 {
    return false;
  }
  is_tty(fd)
}

#[cfg(unix)]
fn is_tty(fd: i32) -> bool {
  // SAFETY: We're checking if the fd is a terminal.
  // The fd may or may not be valid, but libc::isatty handles that safely.
  unsafe { libc::isatty(fd) == 1 }
}

#[cfg(windows)]
fn is_tty(fd: i32) -> bool {
  use winapi::um::consoleapi::GetConsoleMode;
  use winapi::um::processenv::GetStdHandle;
  use winapi::um::winbase::STD_ERROR_HANDLE;
  use winapi::um::winbase::STD_INPUT_HANDLE;
  use winapi::um::winbase::STD_OUTPUT_HANDLE;

  // SAFETY: GetStdHandle returns a borrowed handle to stdin/stdout/stderr.
  // For fd > 2, we try to use it as a raw handle directly.
  let handle = match fd {
    // SAFETY: These are valid standard handles.
    0 => unsafe { GetStdHandle(STD_INPUT_HANDLE) },
    // SAFETY: These are valid standard handles.
    1 => unsafe { GetStdHandle(STD_OUTPUT_HANDLE) },
    // SAFETY: These are valid standard handles.
    2 => unsafe { GetStdHandle(STD_ERROR_HANDLE) },
    _ => fd as winapi::um::winnt::HANDLE,
  };

  let mut mode = 0;
  // SAFETY: handle is either a valid standard handle or a raw fd cast to HANDLE.
  // GetConsoleMode will return 0 if the handle is invalid or not a console.
  unsafe { GetConsoleMode(handle, &mut mode) != 0 }
}

#[op2(fast)]
pub fn op_node_guess_handle_type(state: &mut OpState, rid: u32) -> u32 {
  let handle = match state.resource_table.get_handle(rid) {
    Ok(handle) => handle,
    _ => return HandleType::Unknown as u32,
  };

  let handle_type = match handle {
    ResourceHandle::Fd(handle) => guess_handle_type(handle),
    _ => HandleType::Unknown,
  };

  handle_type as u32
}

#[cfg(windows)]
fn guess_handle_type(handle: ResourceHandleFd) -> HandleType {
  use winapi::um::consoleapi::GetConsoleMode;
  use winapi::um::fileapi::GetFileType;
  use winapi::um::winbase::FILE_TYPE_CHAR;
  use winapi::um::winbase::FILE_TYPE_DISK;
  use winapi::um::winbase::FILE_TYPE_PIPE;

  // SAFETY: Call to win32 fileapi. `handle` is a valid fd.
  match unsafe { GetFileType(handle) } {
    FILE_TYPE_DISK => HandleType::File,
    FILE_TYPE_CHAR => {
      let mut mode = 0;
      // SAFETY: Call to win32 consoleapi. `handle` is a valid fd.
      //         `mode` is a valid pointer.
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

#[cfg(unix)]
fn guess_handle_type(handle: ResourceHandleFd) -> HandleType {
  use std::io::IsTerminal;
  // SAFETY: The resource remains open for the duration of borrow_raw.
  if unsafe { std::os::fd::BorrowedFd::borrow_raw(handle).is_terminal() } {
    return HandleType::Tty;
  }

  // SAFETY: It is safe to zero-initialize a `libc::stat` struct.
  let mut s = unsafe { std::mem::zeroed() };
  // SAFETY: Call to libc
  if unsafe { libc::fstat(handle, &mut s) } == 1 {
    return HandleType::Unknown;
  }

  match s.st_mode & 61440 {
    libc::S_IFREG | libc::S_IFCHR => HandleType::File,
    libc::S_IFIFO => HandleType::Pipe,
    libc::S_IFSOCK => HandleType::Tcp,
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
    let name = script.to_rust_string_lossy(scope);

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
) -> Result<v8::Local<'s, v8::Array>, JsErrorBox> {
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

  obj
    .get_property_names(
      scope,
      v8::GetPropertyNamesArgs {
        index_filter: v8::IndexFilter::SkipIndices,
        property_filter,
        key_conversion: v8::KeyConversionMode::NoNumbers,
        mode: v8::KeyCollectionMode::OwnOnly,
      },
    )
    .ok_or_else(|| {
      JsErrorBox::type_error("Failed to get own non-index properties")
    })
}

#[op2]
pub fn op_node_parse_env<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[string] content: &str,
) -> v8::Local<'a, v8::Object> {
  let env_obj = v8::Object::new(scope);
  parse_env_content_hook(content, |key, value| {
    let key = v8::String::new(scope, key).unwrap();
    let value = v8::String::new(scope, value).unwrap();
    env_obj.set(scope, key.into(), value.into());
  });
  env_obj
}
