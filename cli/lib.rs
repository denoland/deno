#![allow(unused)]

pub mod args;
pub mod cache;
pub mod cdp;
pub mod factory;
pub mod file_fetcher;
pub mod graph_container;
pub mod graph_util;
pub mod http_util;
pub mod jsr;
pub mod lsp;
pub mod module_loader;
pub mod node;
pub mod npm;
pub mod ops;
pub mod registry;
pub mod resolver;
pub mod standalone;
pub mod task_runner;
pub mod tools;
pub mod tsc;
pub mod type_checker;
pub mod util;
pub mod worker;

use std::path::PathBuf;

use deno_core::unsync::JoinHandle;
use deno_core::futures::FutureExt;
pub use deno_lib::worker::LibWorkerFactoryRoots;
use deno_runtime::UnconfiguredRuntime;
pub use deno_terminal::colors;
pub use factory::CliFactory;
pub use deno_runtime::tokio_util::create_and_run_current_thread_with_maybe_metrics;
pub use deno_core::error::AnyError;

use crate::util::v8::init_v8_flags;
use crate::util::v8::get_v8_flags_from_env;
use crate::args::get_default_v8_flags;
use crate::args::Flags;
pub use crate::util::display;

pub mod sys {
  #[allow(clippy::disallowed_types)] // ok, definition
  pub type CliSys = sys_traits::impls::RealSys;
}

pub(crate) fn unstable_exit_cb(feature: &str, api_name: &str) {
  log::error!(
    "Unstable API '{api_name}'. The `--unstable-{}` flag must be provided.",
    feature
  );
  deno_runtime::exit(70);
}


/// Ensures that all subcommands return an i32 exit code and an [`AnyError`] error type.
pub trait SubcommandOutput {
  fn output(self) -> Result<i32, AnyError>;
}

impl SubcommandOutput for Result<i32, AnyError> {
  fn output(self) -> Result<i32, AnyError> {
    self
  }
}

impl SubcommandOutput for Result<(), AnyError> {
  fn output(self) -> Result<i32, AnyError> {
    self.map(|_| 0)
  }
}

impl SubcommandOutput for Result<(), std::io::Error> {
  fn output(self) -> Result<i32, AnyError> {
    self.map(|_| 0).map_err(|e| e.into())
  }
}

/// Ensure that the subcommand runs in a task, rather than being directly executed. Since some of these
/// futures are very large, this prevents the stack from getting blown out from passing them by value up
/// the callchain (especially in debug mode when Rust doesn't have a chance to elide copies!).
#[inline(always)]
pub fn spawn_subcommand<F: Future<Output = T> + 'static, T: SubcommandOutput>(
  f: F,
) -> JoinHandle<Result<i32, AnyError>> {
  // the boxed_local() is important in order to get windows to not blow the stack in debug
  deno_core::unsync::spawn(
    async move { f.map(|r| r.output()).await }.boxed_local(),
  )
}

pub fn init_v8(flags: &Flags) {
  let default_v8_flags = get_default_v8_flags();

  let env_v8_flags = get_v8_flags_from_env();
  let is_single_threaded = env_v8_flags
    .iter()
    .chain(&flags.v8_flags)
    .any(|flag| flag == "--single-threaded");
  init_v8_flags(&default_v8_flags, &flags.v8_flags, env_v8_flags);
  let v8_platform = if is_single_threaded {
    Some(::deno_core::v8::Platform::new_single_threaded(true).make_shared())
  } else {
    None
  };

  deno_core::JsRuntime::init_platform(v8_platform);
}

#[cfg(unix)]
#[allow(clippy::type_complexity)]
pub fn wait_for_start(
  args: &[std::ffi::OsString],
  roots: LibWorkerFactoryRoots,
) -> Option<
  impl Future<
    Output = Result<
      Option<(UnconfiguredRuntime, Vec<std::ffi::OsString>, PathBuf)>,
      AnyError,
    >,
  > + use<>,
> {
  let startup_snapshot = deno_snapshots::CLI_SNAPSHOT?;
  let addr = std::env::var("DENO_UNSTABLE_CONTROL_SOCK").ok()?;

  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::remove_var("DENO_UNSTABLE_CONTROL_SOCK")
  };

  let argv0 = args[0].clone();

  Some(async move {
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncRead;
    use tokio::io::AsyncWrite;
    use tokio::io::AsyncWriteExt;
    use tokio::io::BufReader;
    use tokio::net::TcpListener;
    use tokio::net::UnixSocket;
    use crate::args::Flags;

    #[cfg(any(
      target_os = "android",
      target_os = "linux",
      target_os = "macos"
    ))]
    use tokio_vsock::VsockAddr;
    #[cfg(any(
      target_os = "android",
      target_os = "linux",
      target_os = "macos"
    ))]
    use tokio_vsock::VsockListener;


    init_v8(&Flags::default());

    let unconfigured = deno_runtime::UnconfiguredRuntime::new::<
      deno_resolver::npm::DenoInNpmPackageChecker,
      crate::npm::CliNpmResolver,
      crate::sys::CliSys,
    >(deno_runtime::UnconfiguredRuntimeOptions {
      startup_snapshot,
      create_params: deno_lib::worker::create_isolate_create_params(
        &crate::sys::CliSys::default(),
      ),
      shared_array_buffer_store: Some(roots.shared_array_buffer_store.clone()),
      compiled_wasm_module_store: Some(
        roots.compiled_wasm_module_store.clone(),
      ),
      additional_extensions: vec![],
    });

    let (rx, mut tx): (
      Box<dyn AsyncRead + Unpin>,
      Box<dyn AsyncWrite + Send + Unpin>,
    ) = match addr.split_once(':') {
      Some(("tcp", addr)) => {
        let listener = TcpListener::bind(addr).await?;
        let (stream, _) = listener.accept().await?;
        let (rx, tx) = stream.into_split();
        (Box::new(rx), Box::new(tx))
      }
      Some(("unix", path)) => {
        let socket = UnixSocket::new_stream()?;
        socket.bind(path)?;
        let listener = socket.listen(1)?;
        let (stream, _) = listener.accept().await?;
        let (rx, tx) = stream.into_split();
        (Box::new(rx), Box::new(tx))
      }
      #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos"
      ))]
      Some(("vsock", addr)) => {
        let Some((cid, port)) = addr.split_once(':') else {
          deno_core::anyhow::bail!("invalid vsock addr");
        };
        let cid = if cid == "-1" { u32::MAX } else { cid.parse()? };
        let port = port.parse()?;
        let addr = VsockAddr::new(cid, port);
        let listener = VsockListener::bind(addr)?;
        let (stream, _) = listener.accept().await?;
        let (rx, tx) = stream.into_split();
        (Box::new(rx), Box::new(tx))
      }
      _ => {
        deno_core::anyhow::bail!("invalid control sock");
      }
    };

    let mut buf = Vec::with_capacity(1024);
    BufReader::new(rx).read_until(b'\n', &mut buf).await?;

    tokio::spawn(async move {
      deno_runtime::deno_http::SERVE_NOTIFIER.notified().await;

      #[derive(deno_core::serde::Serialize)]
      enum Event {
        Serving,
      }

      let mut buf = deno_core::serde_json::to_vec(&Event::Serving).unwrap();
      buf.push(b'\n');
      let _ = tx.write_all(&buf).await;
    });

    #[derive(deno_core::serde::Deserialize)]
    struct Start {
      cwd: String,
      args: Vec<String>,
      env: Vec<(String, String)>,
    }

    let cmd: Start = deno_core::serde_json::from_slice(&buf)?;

    std::env::set_current_dir(&cmd.cwd)?;

    for (k, v) in cmd.env {
      // SAFETY: We're doing this before any threads are created.
      unsafe { std::env::set_var(k, v) };
    }

    let args = [argv0]
      .into_iter()
      .chain(cmd.args.into_iter().map(Into::into))
      .collect();

    Ok(Some((unconfigured, args, PathBuf::from(cmd.cwd))))
  })
}