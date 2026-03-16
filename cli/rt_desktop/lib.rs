// Copyright 2018-2026 the Deno authors. MIT license.

//! Desktop runtime for Deno (libdenort).
//!
//! This is a cdylib that exports the WEF C ABI (wef_runtime_init,
//! wef_runtime_start, wef_runtime_shutdown) and boots the full Deno
//! standalone runtime. A WEF backend (CEF, WebView, Servo) loads this
//! shared library and provides the browser/window layer.
//!
//! The user's code uses `Deno.serve()` or `export default { fetch }`
//! to serve an HTTP app. The desktop runtime starts it on a local port
//! and navigates the webview to it.

use std::borrow::Cow;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::v8;
use deno_lib::util::result::js_error_downcast_ref;
use deno_lib::version::otel_runtime_config;
use deno_runtime::fmt_errors::format_js_error;
use deno_terminal::colors;
use tokio::sync::oneshot::error::RecvError;
use wef::Value;
use denort::run::RunOptions;

/// Port used for the embedded HTTP server.
const DESKTOP_SERVE_PORT: u16 = 41520;

/// WEF-backed implementation of [`denort::desktop::DesktopApi`].
struct WefDesktopApi;

impl denort::desktop::DesktopApi for WefDesktopApi {
  fn set_title(&self, title: &str) {
    wef::set_title(title);
  }

  fn get_window_size(&self) -> (i32, i32) {
    wef::get_window_size()
  }

  fn set_window_size(&self, width: i32, height: i32) {
    wef::set_window_size(width, height);
  }

  fn get_window_position(&self) -> (i32, i32) {
    wef::get_window_position()
  }

  fn set_window_position(&self, width: i32, height: i32) {
    wef::set_window_position(width, height);
  }

  fn is_resizeable(&self) -> bool {
    wef::is_resizable()
  }

  fn set_resizeable(&self, resizeable: bool) {
    wef::set_resizable(resizeable);
  }

  fn is_always_on_top(&self) -> bool {
    wef::is_always_on_top()
  }

  fn set_always_on_top(&self, always_on_top: bool) {
    wef::set_always_on_top(always_on_top);
  }

  fn is_visible(&self) -> bool {
    wef::is_visible()
  }

  fn show(&self) {
    wef::show();
  }

  fn hide(&self) {
    wef::hide();
  }

  fn focus(&self) {
    wef::focus();
  }

  fn bind(
    &self,
    name: &str,
    scope: &mut v8::PinScope<'_, '_>,
    this: v8::Global<v8::Object>,
    cb: v8::Local<v8::Function>,
  ) {
    wef::bind(name, |mut js_call| {
      let this = v8::Local::new(scope, this);
      let args = std::mem::take(&mut js_call.args)
        .into_iter()
        .map(|v| wef_value_to_v8(scope, v))
        .collect::<Vec<_>>();

      let tc = std::pin::pin!(v8::TryCatch::new(scope));
      let mut tc = &tc.init();

      if let Some(ret) = cb.call(tc, this.into(), &args) {
        // Attach .then()/.catch() to resolve/reject the JsCall
        // when the promise settles.
        if ret.is_promise() {
          let promise: v8::Local<v8::Promise> = ret.try_into().unwrap();

          // Box the JsCall into an Option so either handler can take
          // ownership exactly once. Store as v8::External data.
          let js_call_ptr =
            Box::into_raw(Box::new(Some(js_call))) as *mut std::ffi::c_void;
          let external = v8::External::new(tc, js_call_ptr);

          let on_fulfilled = v8::Function::builder(
            |scope: &mut v8::PinScope,
             args: v8::FunctionCallbackArguments,
             _rv: v8::ReturnValue| {
              let data =
                v8::Local::<v8::External>::try_from(args.data()).unwrap();
              let js_call = unsafe {
                Box::<Option<wef::JsCall>>::from_raw(
                  data.value() as *mut Option<wef::JsCall>
                )
              };
              if let Some(call) = *js_call {
                call.resolve(v8_to_wef_value(scope, args.get(0)));
              }
            },
          )
          .data(external.into())
          .build(tc)
          .unwrap();

          let on_rejected = v8::Function::builder(
            |scope: &mut v8::PinScope,
             args: v8::FunctionCallbackArguments,
             _rv: v8::ReturnValue| {
              let data =
                v8::Local::<v8::External>::try_from(args.data()).unwrap();
              let js_call = unsafe {
                Box::<Option<wef::JsCall>>::from_raw(
                  data.value() as *mut Option<wef::JsCall>
                )
              };
              if let Some(call) = *js_call {
                call.reject(v8_to_wef_value(scope, args.get(0)));
              }
            },
          )
          .data(external.into())
          .build(tc)
          .unwrap();

          promise.then2(tc, on_fulfilled, on_rejected);
        } else {
          js_call.resolve(v8_to_wef_value(tc, ret));
        }
      } else if let Some(err) = tc.exception() {
        js_call.reject(v8_to_wef_value(tc, err));
      } else {
        let message = v8::String::new(tc, "unknown error").unwrap();
        let err = v8::Exception::error(tc, message.into());
        js_call.reject(v8_to_wef_value(tc, err.into()));
      }
    });
  }

  fn unbind(&self, name: &str) {
    wef::unbind(name);
  }

  fn navigate(&self, url: &str) {
    wef::navigate(url);
  }

  async fn execute_js(&self, scope: &mut v8::PinScope<'_, '_>, script: &str) -> Result<v8::Local<v8::Value>, v8::Local<v8::Value>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    wef::execute_js(script, Some(|res| {
      let _ = tx.send(res);
    }));

    match rx.await.expect("execute_js channel failed") {
      Ok(val) => Ok(wef_value_to_v8(scope, val)),
      Err(err) => Err(wef_value_to_v8(scope, err)),
    }
  }

  fn quit(&self) {
    wef::quit();
  }

  fn set_application_menu(&self, template_json: &str) {
    let json: serde_json::Value = match serde_json::from_str(template_json) {
      Ok(v) => v,
      Err(e) => {
        log::error!("Invalid menu template JSON: {}", e);
        return;
      }
    };
    let wef_value = json_to_wef_value(&json);
    wef::set_application_menu_raw(wef_value);
  }
}

fn wef_value_to_v8(
  scope: &v8::PinScope<'_, '_>,
  val: wef::Value,
) -> v8::Local<v8::Value> {
  match val {
    wef::Value::Null => v8::null(scope).into(),
    wef::Value::Bool(bool) => v8::Boolean::new(scope, bool).into(),
    wef::Value::Int(int) => v8::Integer::new(scope, int).into(),
    wef::Value::Double(double) => v8::Number::new(scope, double).into(),
    wef::Value::String(str) => v8::String::new(scope, &str).into(),
    wef::Value::List(list) => {
      let elements = list
        .into_iter()
        .map(|v| wef_value_to_v8(scope, v))
        .collect::<Vec<_>>();
      v8::Array::new_with_elements(scope, &elements).into()
    }
    wef::Value::Dict(dict) => {
      let mut names = Vec::with_capacity(dict.len());
      let mut values = Vec::with_capacity(dict.len());

      for (k, v) in dict {
        names.push(v8::String::new(scope, &k).into());
        values.push(wef_value_to_v8(scope, v));
      }

      let prototype = v8::null(scope).into();
      v8::Object::with_prototype_and_properties(
        scope, prototype, &names, &values,
      )
      .into()
    }
    wef::Value::Binary(bin) => {
      let len = bin.len();
      let backing_store = v8::ArrayBuffer::new_backing_store_from_vec(bin);
      let backing_store = backing_store.make_shared();
      let ab = v8::ArrayBuffer::with_backing_store(scope, &backing_store);
      let uint8_array = v8::Uint8Array::new(scope, ab, 0, len).unwrap();
      uint8_array.into()
    }
  }
}

fn v8_to_wef_value(
  scope: &v8::PinScope<'_, '_>,
  val: v8::Local<v8::Value>,
) -> wef::Value {
  if val.is_null_or_undefined() {
    wef::Value::Null
  } else if val.is_boolean() {
    wef::Value::Bool(val.boolean_value(scope))
  } else if val.is_int32() {
    wef::Value::Int(val.int32_value(scope).unwrap_or(0))
  } else if val.is_number() {
    wef::Value::Double(val.number_value(scope).unwrap_or(0.0))
  } else if val.is_string() {
    let s = val.to_rust_string_lossy(scope);
    wef::Value::String(s)
  } else if val.is_array_buffer_view() {
    let view: v8::Local<v8::ArrayBufferView> = val.try_into().unwrap();
    let len = view.byte_length();
    let mut buf = vec![0u8; len];
    view.copy_contents(&mut buf);
    wef::Value::Binary(buf)
  } else if val.is_array() {
    let arr: v8::Local<v8::Array> = val.try_into().unwrap();
    let len = arr.length();
    let mut list = Vec::with_capacity(len as usize);
    for i in 0..len {
      if let Some(elem) = arr.get_index(scope, i) {
        list.push(v8_to_wef_value(scope, elem));
      }
    }
    wef::Value::List(list)
  } else if val.is_object() {
    let obj: v8::Local<v8::Object> = val.try_into().unwrap();
    let mut map = std::collections::HashMap::new();
    if let Some(names) =
      obj.get_own_property_names(scope, v8::GetPropertyNamesArgs::default())
    {
      for i in 0..names.length() {
        if let Some(key) = names.get_index(scope, i) {
          let key_str = key.to_rust_string_lossy(scope);
          if let Some(value) = obj.get(scope, key) {
            map.insert(key_str, v8_to_wef_value(scope, value));
          }
        }
      }
    }
    wef::Value::Dict(map)
  } else {
    // Fallback: coerce to string
    wef::Value::String(val.to_rust_string_lossy(scope))
  }
}

/// Promote this dylib's symbols to the global symbol scope so that
/// native addons loaded via `dlopen` (e.g. next-swc.node) can resolve
/// NAPI function symbols from our library.
///
/// By default, WEF loads this dylib without `RTLD_GLOBAL`, so its symbols
/// are only visible within the dylib itself. NAPI addons use
/// `-undefined dynamic_lookup` (macOS) and expect NAPI symbols to be
/// in the global symbol table. Re-opening ourselves with `RTLD_GLOBAL`
/// promotes our exports to global scope.
#[cfg(unix)]
fn promote_dylib_symbols_to_global() {
  #[repr(C)]
  struct DlInfo {
    dli_fname: *const std::ffi::c_char,
    dli_fbase: *mut std::ffi::c_void,
    dli_sname: *const std::ffi::c_char,
    dli_saddr: *mut std::ffi::c_void,
  }
  unsafe extern "C" {
    fn dladdr(
      addr: *const std::ffi::c_void,
      info: *mut DlInfo,
    ) -> std::ffi::c_int;
    fn dlopen(
      path: *const std::ffi::c_char,
      flags: std::ffi::c_int,
    ) -> *mut std::ffi::c_void;
  }
  const RTLD_LAZY: std::ffi::c_int = 0x1;
  const RTLD_NOLOAD: std::ffi::c_int = 0x10;
  const RTLD_GLOBAL: std::ffi::c_int = 0x8;

  unsafe {
    let mut info: DlInfo = std::mem::zeroed();
    // Use a function in our dylib as a reference address
    let addr = promote_dylib_symbols_to_global as *const std::ffi::c_void;
    if dladdr(addr, &mut info) != 0 && !info.dli_fname.is_null() {
      // Re-open our own dylib with RTLD_GLOBAL to promote symbols
      dlopen(info.dli_fname, RTLD_LAZY | RTLD_NOLOAD | RTLD_GLOBAL);
    }
  }
}

/// Get the filesystem path of this dylib using `dladdr`.
#[cfg(unix)]
fn get_dylib_path() -> Option<PathBuf> {
  #[repr(C)]
  struct DlInfo {
    dli_fname: *const std::ffi::c_char,
    dli_fbase: *mut std::ffi::c_void,
    dli_sname: *const std::ffi::c_char,
    dli_saddr: *mut std::ffi::c_void,
  }
  unsafe extern "C" {
    fn dladdr(
      addr: *const std::ffi::c_void,
      info: *mut DlInfo,
    ) -> std::ffi::c_int;
  }
  unsafe {
    let mut info: DlInfo = std::mem::zeroed();
    let addr = get_dylib_path as *const std::ffi::c_void;
    if dladdr(addr, &mut info) != 0 && !info.dli_fname.is_null() {
      let c_str = std::ffi::CStr::from_ptr(info.dli_fname);
      Some(PathBuf::from(c_str.to_string_lossy().into_owned()))
    } else {
      None
    }
  }
}

/// Manages pending updates and rollback on startup.
///
/// Uses a sentinel file (`.update-ok`) to detect if the last update
/// booted successfully:
///
/// - `.update` exists → apply it (current → `.backup`, `.update` → current)
/// - `.backup` exists but `.update-ok` doesn't → last update crashed, rollback
/// - `.backup` exists and `.update-ok` exists → previous update succeeded, clean up
///
/// Returns `true` if a rollback occurred (so we can dispatch an event in JS).
fn apply_pending_update(dylib_path: &Path) -> bool {
  let ext = dylib_path.extension().unwrap_or_default().to_string_lossy();
  let update_path = dylib_path.with_extension(format!("{}.update", ext));
  let backup_path = dylib_path.with_extension(format!("{}.backup", ext));
  let sentinel_path = dylib_path.with_extension(format!("{}.update-ok", ext));

  if update_path.exists() {
    // New update pending — apply it.
    // Remove stale sentinel so we can detect if *this* update fails.
    let _ = std::fs::remove_file(&sentinel_path);
    let _ = std::fs::remove_file(&backup_path);
    if std::fs::rename(dylib_path, &backup_path).is_ok() {
      if std::fs::rename(&update_path, dylib_path).is_err() {
        // Failed to move update into place — rollback immediately.
        let _ = std::fs::rename(&backup_path, dylib_path);
      }
    }
    return false;
  }

  if backup_path.exists() && !sentinel_path.exists() {
    // Last update didn't write the sentinel → it crashed. Rollback.
    eprintln!("[desktop] Last update failed to start, rolling back...");
    let _ = std::fs::rename(&backup_path, dylib_path);
    return true;
  }

  if backup_path.exists() && sentinel_path.exists() {
    // Previous update booted fine — clean up backup and sentinel.
    let _ = std::fs::remove_file(&backup_path);
    let _ = std::fs::remove_file(&sentinel_path);
  }

  false
}

wef::main!(|| {
  // Apply any pending update before anything else.
  #[cfg(unix)]
  let update_rolled_back = {
    match std::panic::catch_unwind(|| {
      if let Some(ref dylib_path) = get_dylib_path() {
        eprintln!("[desktop] dylib path: {:?}", dylib_path);
        apply_pending_update(dylib_path)
      } else {
        eprintln!("[desktop] could not determine dylib path");
        false
      }
    }) {
      Ok(v) => v,
      Err(e) => {
        eprintln!("[desktop] update check panicked: {:?}", e);
        false
      }
    }
  };
  #[cfg(not(unix))]
  let update_rolled_back = false;

  // Make NAPI symbols visible to native addons (e.g. next-swc).
  #[cfg(unix)]
  promote_dylib_symbols_to_global();

  // Guard against re-entry: when a framework dev server (e.g. Next.js)
  // forks child/worker processes, they re-execute this dylib. Detect
  // forked workers and run them headless (no WEF window) by checking
  // for NODE_CHANNEL_FD (set by Node's child_process.fork()) or
  // NEXT_PRIVATE_WORKER (set by Next.js specifically).
  let args: Vec<_> = env::args_os().collect();
  let is_worker = env::var("NODE_CHANNEL_FD").is_ok()
    || env::var("NEXT_PRIVATE_WORKER").is_ok()
    || extract_fork_script_path(&args).is_some();
  if is_worker {
    run_headless_worker();
    return;
  }

  denort::init_logging(None, None);

  deno_runtime::deno_permissions::mark_standalone();

  rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .unwrap();

  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  rt.block_on(async {
    eprintln!("[desktop] run_desktop starting");
    match run_desktop(update_rolled_back).await {
      Ok(()) => eprintln!("[desktop] run_desktop completed OK"),
      Err(error) => {
        let error_string = match js_error_downcast_ref(&error) {
          Some(js_error) => format_js_error(js_error, None),
          None => format!("{:?}", error),
        };
        log::error!(
          "{}: {}",
          colors::red_bold("error"),
          error_string.trim_start_matches("error: ")
        );
      }
    }
  });
});

/// Run as a headless worker (no WEF window). Used when a framework dev
/// server forks child processes that re-execute this dylib.
fn run_headless_worker() {
  denort::init_logging(None, None);
  deno_runtime::deno_permissions::mark_standalone();
  rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .unwrap();

  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  rt.block_on(async {
    let args: Vec<_> = env::args_os().collect();
    eprintln!("[worker] args: {:?}", args);
    eprintln!("[worker] cwd: {:?}", env::current_dir());

    // Detect if this is a child_process.fork() invocation.
    // fork() translates args to: ["run", "-A", "--unstable-...", "script.js", ...]
    // Extract the script path so the forked worker runs the correct module
    // instead of the embedded entrypoint.
    let fork_module = extract_fork_script_path(&args);
    eprintln!("[worker] fork_module: {:?}", fork_module);

    let data = match denort::binary::extract_standalone_with_finder(
      Cow::Owned(args),
      find_section_in_dylib,
    ) {
      Ok(data) => data,
      Err(e) => {
        log::error!("Worker failed to load standalone data: {:?}", e);
        return;
      }
    };

    denort::load_env_vars(&data.metadata.env_vars_from_env_file);

    let sys = if data.metadata.self_extracting.is_some() {
      // VFS should already be extracted by the parent process.
      // In dev mode, keep the source directory as CWD (inherited from parent).
      // In production mode, set CWD to extraction directory.
      if env::var("DENO_DESKTOP_DEV").is_err() {
        let _ = std::env::set_current_dir(&data.root_path);
      }
      denort::file_system::DenoRtSys::new_self_extracting(data.vfs.clone())
    } else {
      denort::file_system::DenoRtSys::new(data.vfs.clone())
    };

    let options = denort::run::RunOptions {
      override_main_module: fork_module,
      ..Default::default()
    };

    eprintln!("[worker] starting run_with_options");

    // Log worker output to a file for debugging since the parent may
    // call process.exit() and kill our stderr.
    let log_path = std::env::temp_dir().join("deno_desktop_worker.log");
    let log_msg = format!("[worker] log file: {:?}\n", log_path);
    eprint!("{}", log_msg);
    let _ = std::fs::write(&log_path, &log_msg);

    match denort::run::run_with_options(
      Arc::new(sys.clone()),
      sys,
      data,
      options,
    )
    .await
    {
      Ok(exit_code) => {
        let msg = format!(
          "[worker] run_with_options completed with exit code: {}\n",
          exit_code
        );
        eprint!("{}", msg);
        let _ = std::fs::OpenOptions::new()
          .append(true)
          .open(&log_path)
          .and_then(|mut f| std::io::Write::write_all(&mut f, msg.as_bytes()));
      }
      Err(error) => {
        let error_string = match js_error_downcast_ref(&error) {
          Some(js_error) => format_js_error(js_error, None),
          None => format!("{:?}", error),
        };
        let msg =
          format!("[worker] run_with_options error: {}\n", error_string);
        eprint!("{}", msg);
        let _ = std::fs::OpenOptions::new()
          .append(true)
          .open(&log_path)
          .and_then(|mut f| std::io::Write::write_all(&mut f, msg.as_bytes()));
        log::error!(
          "{}: {}",
          colors::red_bold("error"),
          error_string.trim_start_matches("error: ")
        );
      }
    }
    eprintln!("[worker] block_on finished");
  });
  eprintln!("[worker] run_headless_worker returning");
}

/// Extract the script path from fork'd process arguments.
///
/// When `child_process.fork(scriptPath)` is called, the args are translated
/// to Deno CLI args: `["<exe>", "run", "-A", "--unstable-...", "script.js", ...]`
/// This function finds the script path (first non-flag arg after "run").
fn extract_fork_script_path(
  args: &[std::ffi::OsString],
) -> Option<deno_core::url::Url> {
  let args: Vec<String> = args
    .iter()
    .filter_map(|a| a.to_str().map(String::from))
    .collect();

  // Skip argv[0] (the executable), expect "run" as the subcommand
  if args.len() < 3 || args[1] != "run" {
    return None;
  }

  // Find the first arg after "run" that isn't a flag
  for arg in &args[2..] {
    if arg.starts_with('-') {
      continue;
    }
    // This is the script path
    let path = PathBuf::from(arg);
    let path = if path.is_absolute() {
      path
    } else {
      env::current_dir().ok()?.join(path)
    };
    return deno_core::url::Url::from_file_path(path).ok();
  }
  None
}

/// Convert a serde_json::Value to a wef::Value for the menu template.
fn json_to_wef_value(v: &serde_json::Value) -> wef::Value {
  match v {
    serde_json::Value::Null => wef::Value::Null,
    serde_json::Value::Bool(b) => wef::Value::Bool(*b),
    serde_json::Value::Number(n) => {
      if let Some(i) = n.as_i64() {
        wef::Value::Int(i as i32)
      } else {
        wef::Value::Double(n.as_f64().unwrap_or(0.0))
      }
    }
    serde_json::Value::String(s) => wef::Value::String(s.clone()),
    serde_json::Value::Array(arr) => {
      wef::Value::List(arr.iter().map(json_to_wef_value).collect())
    }
    serde_json::Value::Object(obj) => {
      let mut map = std::collections::HashMap::new();
      for (k, v) in obj {
        map.insert(k.clone(), json_to_wef_value(v));
      }
      wef::Value::Dict(map)
    }
  }
}

/// Find the embedded data section in this dylib (not the main executable).
fn find_section_in_dylib() -> Result<&'static [u8], AnyError> {
  match libsui::find_section_in_current_image("d3n0l4nd")
    .context("Failed reading standalone binary section from dylib.")
  {
    Ok(Some(data)) => Ok(data),
    Ok(None) => {
      bail!("Could not find standalone binary section in dylib.")
    }
    Err(err) => Err(err),
  }
}

async fn run_desktop(update_rolled_back: bool) -> Result<(), AnyError> {
  let args: Vec<_> = env::args_os().collect();
  let data = denort::binary::extract_standalone_with_finder(
    Cow::Owned(args),
    find_section_in_dylib,
  )?;

  deno_runtime::deno_telemetry::init(
    otel_runtime_config(),
    data.metadata.otel_config.clone(),
  )?;
  denort::init_logging(
    data.metadata.log_level,
    Some(data.metadata.otel_config.clone()),
  );
  denort::load_env_vars(&data.metadata.env_vars_from_env_file);

  // Set DENO_SERVE_ADDRESS so Deno.serve() and Node http servers
  // automatically bind to the desktop port.
  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::set_var(
      "DENO_SERVE_ADDRESS",
      format!("tcp:127.0.0.1:{}", DESKTOP_SERVE_PORT),
    );
  }

  let sys = if data.metadata.self_extracting.is_some() {
    denort::binary::extract_vfs_to_disk(&data.vfs, &data.root_path)?;
    // Set CWD to extraction directory so frameworks like Next.js
    // can find their build output (e.g. .next/) relative to CWD.
    std::env::set_current_dir(&data.root_path)?;
    denort::file_system::DenoRtSys::new_self_extracting(data.vfs.clone())
  } else {
    denort::file_system::DenoRtSys::new(data.vfs.clone())
  };

  // Enable HMR if DENO_DESKTOP_HMR is set to a directory path
  // (set by `deno compile --desktop --hmr`).
  let hmr_watch_dir = env::var("DENO_DESKTOP_HMR").ok().map(PathBuf::from);

  // Framework dev servers handle their own HMR via websocket.
  // For non-framework apps, V8-level HMR reloads the webview.
  let is_framework_dev = env::var("DENO_DESKTOP_DEV").is_ok();

  // In dev mode, restore CWD to the source directory so the framework
  // dev server watches the original source files, not the extracted VFS.
  if is_framework_dev {
    if let Ok(source_dir) = env::var("DENO_DESKTOP_HMR") {
      std::env::set_current_dir(&source_dir)?;
    }
  }

  let hmr_on_reload: Option<denort::hmr::HmrReloadCallback> =
    if hmr_watch_dir.is_some() && !is_framework_dev {
      Some(Box::new(|| {
        wef::execute_js("location.reload()", None);
      }))
    } else {
      None
    };

  // Desktop extension: provides Deno.desktop.* APIs + auto-update
  #[cfg(unix)]
  let auto_update_state = get_dylib_path().map(|p| {
    denort::desktop::AutoUpdateState {
      dylib_path: p,
      app_version: data.metadata.app_version.clone(),
      rolled_back: update_rolled_back, // from wef::main! startup check
    }
  });
  #[cfg(not(unix))]
  let auto_update_state: Option<denort::desktop::AutoUpdateState> = None;

  let auto_update_version = auto_update_state
    .as_ref()
    .and_then(|s| s.app_version.clone());
  let auto_update_rolled_back =
    auto_update_state.as_ref().is_some_and(|s| s.rolled_back);

  let run_opts = RunOptions {
    auto_serve: true,
    serve_port: Some(DESKTOP_SERVE_PORT),
    serve_host: Some("127.0.0.1".to_string()),
    hmr_watch_dir: if is_framework_dev {
      None
    } else {
      hmr_watch_dir
    },
    hmr_on_reload,
    op_state_init: Some(Box::new(move |state| {
      denort::desktop::init_desktop_state(
        state,
        Box::new(WefDesktopApi),
        auto_update_state,
      );
      // Wire WEF menu click callback to the Deno runtime channel
      let menu_tx =
        state.borrow::<denort::desktop::MenuClickSender>().0.clone();
      wef::set_menu_click_handler(move |id| {
        let _ = menu_tx.send(id.to_string());
      });
      // Wire WEF keyboard events to the Deno runtime channel
      let kb_tx = state
        .borrow::<denort::desktop::KeyboardEventSender>()
        .0
        .clone();
      wef::on_keyboard_event(move |ev| {
        let _ =
          kb_tx.send(deno_runtime::ops::desktop::KeyboardEventData {
            r#type: match ev.state {
              wef::KeyState::Pressed => "keydown",
              wef::KeyState::Released => "keyup",
            },
            key: ev.key,
            code: ev.code,
            shift: ev.modifiers.shift,
            control: ev.modifiers.control,
            alt: ev.modifiers.alt,
            meta: ev.modifiers.meta,
            repeat: ev.repeat,
          });
      });
    })),
    override_main_module: None,
    auto_update_version,
    auto_update_rolled_back,
  };

  // Run the Deno runtime and WEF event loop concurrently.
  // We spawn the runtime first, wait for the server to be ready,
  // then navigate the webview.
  let url = format!("http://127.0.0.1:{}", DESKTOP_SERVE_PORT);
  eprintln!("[desktop] starting runtime and wef event loop");
  let run_fut =
    denort::run::run_with_options(Arc::new(sys.clone()), sys, data, run_opts);
  let wef_fut = wef::run();

  // Wait for the server to be ready, then navigate the webview.
  // Do a full HTTP request instead of just a TCP connect — frameworks
  // like Vite accept connections before they're ready to serve.
  let navigate_fut = async {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    for i in 0..60 {
      if let Ok(mut stream) =
        tokio::net::TcpStream::connect(("127.0.0.1", DESKTOP_SERVE_PORT)).await
      {
        let req = format!(
          "GET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
          DESKTOP_SERVE_PORT
        );
        if stream.write_all(req.as_bytes()).await.is_ok() {
          let mut buf = vec![0u8; 256];
          if let Ok(n) = stream.read(&mut buf).await {
            let response = String::from_utf8_lossy(&buf[..n]);
            if response.starts_with("HTTP/1.1 2")
              || response.starts_with("HTTP/1.1 3")
              || response.starts_with("HTTP/1.0 2")
              || response.starts_with("HTTP/1.0 3")
            {
              eprintln!(
                "[desktop] Server ready after {} attempts, navigating to {}",
                i + 1,
                &url
              );
              wef::navigate(&url);
              return;
            }
          }
        }
      }
      tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    log::warn!("Server not ready after 15s, navigating anyway");
    wef::navigate(&url);
  };

  tokio::select! {
    result = async {
      // Drive the navigate future alongside the runtime.
      tokio::join!(navigate_fut, run_fut).1
    } => {
      match result {
        Ok(exit_code) => {
          eprintln!("[desktop] Deno runtime exited with code {}", exit_code);
        }
        Err(err) => {
          eprintln!("[desktop] Deno runtime error: {:?}", err);
          return Err(err);
        }
      }
    }
    _ = wef_fut => {
      eprintln!("[desktop] WEF event loop ended (window closed)");
    }
  }

  Ok(())
}
