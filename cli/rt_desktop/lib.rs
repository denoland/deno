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
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_lib::util::result::js_error_downcast_ref;
use deno_lib::version::otel_runtime_config;
use deno_runtime::fmt_errors::format_js_error;
use deno_terminal::colors;
use denort::run::RunOptions;

/// Port used for the embedded HTTP server.
const DESKTOP_SERVE_PORT: u16 = 41520;

/// WEF-backed implementation of [`denort::desktop::DesktopApi`].
struct WefDesktopApi;

impl denort::desktop::DesktopApi for WefDesktopApi {
  fn set_title(&self, title: &str) {
    wef::set_title(title);
  }

  fn set_window_size(&self, width: i32, height: i32) {
    wef::set_window_size(width, height);
  }

  fn navigate(&self, url: &str) {
    wef::navigate(url);
  }

  fn execute_js(&self, script: &str) {
    wef::execute_js(script);
  }

  fn quit(&self) {
    wef::quit();
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

wef::main!(|| {
  // Make NAPI symbols visible to native addons (e.g. next-swc).
  #[cfg(unix)]
  promote_dylib_symbols_to_global();

  // Guard against re-entry: when a framework dev server (e.g. Next.js)
  // forks child/worker processes, they re-execute this dylib. Detect
  // forked workers and run them headless (no WEF window) by checking
  // for NODE_CHANNEL_FD (set by Node's child_process.fork()) or
  // NEXT_PRIVATE_WORKER (set by Next.js specifically).
  let is_worker = env::var("NODE_CHANNEL_FD").is_ok()
    || env::var("NEXT_PRIVATE_WORKER").is_ok();
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
    match run_desktop().await {
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

async fn run_desktop() -> Result<(), AnyError> {
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
        wef::execute_js("location.reload()");
      }))
    } else {
      None
    };

  // Desktop extension: provides Deno.desktop.* APIs
  let desktop_ext = denort::desktop::init_extension(Box::new(WefDesktopApi));

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
    custom_extensions: vec![desktop_ext],
    override_main_module: None,
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
              log::debug!(
                "Server ready after {} attempts, navigating to {}",
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
