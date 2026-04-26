// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;

use deno_core::OpState;
use deno_core::error::JsError;
use deno_core::op2;
use deno_core::v8;
use deno_dotenv::parse_env_content_hook;
use deno_error::JsErrorBox;
use node_resolver::InNpmPackageChecker;
use node_resolver::NpmPackageFolderResolver;

use crate::ExtNodeSys;
use crate::NodeResolverRc;

// === SIGINT Watchdog ===
// Used by `internalBinding('contextify')` to detect SIGINT during script
// evaluation (e.g. in the REPL). Reference-counted so nested start/stop
// pairs work correctly.

static SIGINT_WATCHDOG_REFCOUNT: AtomicI32 = AtomicI32::new(0);
static SIGINT_WATCHDOG_PENDING: AtomicBool = AtomicBool::new(false);

#[op2(fast)]
pub fn op_node_start_sigint_watchdog() {
  let prev = SIGINT_WATCHDOG_REFCOUNT.fetch_add(1, Ordering::SeqCst);
  if prev == 0 {
    SIGINT_WATCHDOG_PENDING.store(false, Ordering::SeqCst);
    // Use set_interceptor so the watchdog consumes SIGINT exclusively —
    // other listeners (test runner, Deno.addSignalListener) won't see it.
    deno_signals::set_interceptor(
      libc::SIGINT,
      Some(Box::new(|| {
        SIGINT_WATCHDOG_PENDING.store(true, Ordering::SeqCst);
        true // consume the signal
      })),
    );
  }
}

#[op2(fast)]
pub fn op_node_stop_sigint_watchdog() -> bool {
  let prev = SIGINT_WATCHDOG_REFCOUNT.fetch_sub(1, Ordering::SeqCst);
  let had_pending = SIGINT_WATCHDOG_PENDING.swap(false, Ordering::SeqCst);
  if prev == 1 {
    deno_signals::set_interceptor(libc::SIGINT, None);
  }
  had_pending
}

#[op2(fast)]
pub fn op_node_watchdog_has_pending_sigint() -> bool {
  SIGINT_WATCHDOG_PENDING.load(Ordering::SeqCst)
}

// === setTraceSigInt ===
// When enabled, SIGINT triggers a V8 interrupt that captures the current
// JavaScript stack trace, prints it to stderr, and exits with code 130.

static SIGINT_TRACE_HANDLER_ID: Mutex<Option<u32>> = Mutex::new(None);

#[op2(fast)]
pub fn op_node_set_trace_sigint(
  scope: &mut v8::PinScope<'_, '_>,
  enable: bool,
) {
  let mut handler_id = SIGINT_TRACE_HANDLER_ID.lock().unwrap();
  if enable {
    if handler_id.is_some() {
      return;
    }
    let isolate_handle = scope.thread_safe_handle();
    let context = scope.get_current_context();
    let context_global = v8::Global::new(scope, context);
    // Intentionally leaked — the interrupt callback may fire asynchronously
    // after a future unregister call, so we cannot safely free this.
    let context_ptr = Box::into_raw(Box::new(context_global)) as usize;

    let id = deno_signals::register(
      libc::SIGINT,
      true,
      Box::new(move || {
        isolate_handle.request_interrupt(
          sigint_trace_interrupt_callback,
          context_ptr as *mut std::ffi::c_void,
        );
      }),
    )
    .expect("failed to register SIGINT trace handler");
    *handler_id = Some(id);
  } else if let Some(id) = handler_id.take() {
    deno_signals::unregister(libc::SIGINT, id);
  }
}

#[allow(clippy::print_stderr, reason = "intentional stderr for SIGINT trace")]
unsafe extern "C" fn sigint_trace_interrupt_callback(
  isolate_ptr: v8::UnsafeRawIsolatePtr,
  data: *mut std::ffi::c_void,
) {
  let mut raw_ptr = isolate_ptr;
  // SAFETY: V8 guarantees the isolate pointer is valid during the interrupt
  // callback and we are on the V8 thread.
  let isolate =
    unsafe { v8::Isolate::ref_from_raw_isolate_ptr_mut(&mut raw_ptr) };
  // SAFETY: data points to a leaked Box<v8::Global<v8::Context>> that we
  // allocated in op_node_set_trace_sigint. We intentionally don't free it
  // since we exit immediately after.
  let context_global = unsafe { &*(data as *const v8::Global<v8::Context>) };

  v8::scope!(scope, isolate);
  let context = v8::Local::new(scope, context_global);
  let scope = &mut v8::ContextScope::new(scope, context);

  let msg = v8::String::new(scope, "SIGINT").unwrap();
  let exception = v8::Exception::error(scope, msg);

  let js_error = JsError::from_v8_exception(scope, exception);
  eprintln!("{js_error}");
  std::process::exit(130);
}

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
