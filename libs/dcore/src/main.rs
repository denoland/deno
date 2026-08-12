// Copyright 2018-2026 the Deno authors. MIT license.

#![allow(clippy::print_stdout, reason = "example code")]
#![allow(clippy::print_stderr, reason = "example code")]
#![allow(clippy::disallowed_methods, reason = "example code")]

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use anyhow::bail;
use deno_core::RuntimeOptions;
use deno_core::anyhow::Error;
use deno_core_testing::create_runtime_from_snapshot;

static SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/SNAPSHOT.bin"));

mod inspector_server;
mod metrics;
use crate::inspector_server::InspectorServer;
use crate::metrics::create_metrics;

fn main() -> Result<(), Error> {
  eprintln!(
    "🛑 deno_core binary is meant for development and testing purposes."
  );

  let args = match Args::parse(std::env::args().skip(1)) {
    Ok(Some(args)) => args,
    // `--help` was requested; usage has already been printed.
    Ok(None) => return Ok(()),
    Err(e) => {
      eprintln!("error: {e}");
      eprintln!("\n{USAGE}");
      std::process::exit(1);
    }
  };

  let file_path = args.file_path.clone();
  println!("Run {file_path}");

  let (maybe_inspector_addr, maybe_inspect_mode) = args.inspect().unzip();
  let inspector_server = if maybe_inspector_addr.is_some() {
    // TODO(bartlomieju): make it configurable
    let host = "127.0.0.1:9229".parse::<SocketAddr>().unwrap();
    Some(Arc::new(InspectorServer::new(host, "dcore")?))
  } else {
    None
  };

  init_v8_flags(&args.v8_flags);

  // The tokio runtime must exist and be entered before the `JsRuntime` is
  // created, so that delayed V8 tasks can be scheduled on it.
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()?;
  let _tokio_guard = runtime.enter();

  let (metrics_summary, mut js_runtime, _worker_host_side) =
    if args.strace_ops || args.strace_ops_summary {
      let (summary, op_metrics_factory_fn) =
        create_metrics(args.strace_ops, args.strace_ops_summary);

      let (runtime, worker_host_side) =
        deno_core_testing::create_runtime_from_snapshot_with_options(
          SNAPSHOT,
          inspector_server.is_some(),
          None,
          vec![],
          RuntimeOptions {
            op_metrics_factory_fn: Some(op_metrics_factory_fn),
            ..Default::default()
          },
        );
      (Some(summary), runtime, worker_host_side)
    } else {
      let (runtime, worker_host_side) = create_runtime_from_snapshot(
        SNAPSHOT,
        inspector_server.is_some(),
        None,
        vec![],
      );
      (None, runtime, worker_host_side)
    };

  js_runtime
    .op_state()
    .borrow_mut()
    .put(deno_core::error::InitialCwd(Arc::new(
      deno_core::url::Url::from_directory_path(
        std::env::current_dir().context("Unable to get CWD")?,
      )
      .unwrap(),
    )));

  let main_module: deno_core::url::Url = deno_core::resolve_path(
    &file_path,
    &std::env::current_dir().context("Unable to get CWD")?,
  )?;

  if let Some(inspector_server) = inspector_server.clone() {
    inspector_server.register_inspector(
      main_module.to_string(),
      js_runtime.inspector(),
      matches!(maybe_inspect_mode.unwrap(), InspectMode::WaitForConnection),
    );
  }

  let future = async {
    let mod_id = js_runtime.load_main_es_module(&main_module).await?;
    let result = js_runtime.mod_evaluate(mod_id);
    js_runtime.run_event_loop(Default::default()).await?;
    result.await
  };
  let result = runtime.block_on(future);
  if let Some(summary) = metrics_summary {
    eprintln!("{}", summary.to_json_pretty()?)
  }
  result.map_err(|e| e.into())
}

const USAGE: &str = "\
Usage: dcore [OPTIONS] <file_path>

Arguments:
  <file_path>  A relative or absolute file to a file to run

Options:
      --inspect[=<HOST_AND_PORT>]
          Activate inspector on host:port (default: 127.0.0.1:9229)
      --inspect-brk[=<HOST_AND_PORT>]
          Activate inspector on host:port, wait for debugger to connect and break at the start of user script
      --inspect-wait[=<HOST_AND_PORT>]
          Activate inspector on host:port and wait for debugger to connect before running user code
      --strace-ops
          Output a trace of op execution on stderr
      --strace-ops-summary
          Output a summary of op execution on stderr when program exits
      --v8-flags=<V8_FLAGS>
          To see a list of all available flags use --v8-flags=--help
          Flags can also be set via the DCORE_V8_FLAGS environment variable.
          Any flags set with this flag are appended after the DCORE_V8_FLAGS environment variable
  -h, --help
          Print help";

enum InspectMode {
  Immediate,
  WaitForConnection,
}

/// The three `--inspect*` flags are mutually exclusive and take an optional
/// `=host:port` value; without one they default to `127.0.0.1:9229`.
#[derive(Default)]
struct Args {
  file_path: String,
  /// `Some(None)` means the flag was passed without an explicit address.
  inspect: Option<Option<SocketAddr>>,
  inspect_wait: Option<Option<SocketAddr>>,
  strace_ops: bool,
  strace_ops_summary: bool,
  v8_flags: Vec<String>,
}

impl Args {
  /// Returns `Ok(None)` when `--help` was requested (usage already printed).
  fn parse(
    args: impl IntoIterator<Item = String>,
  ) -> Result<Option<Self>, Error> {
    let mut out = Args::default();
    let mut file_path: Option<String> = None;
    let mut seen_inspect_flag: Option<&'static str> = None;

    // `--inspect*` takes its value only via `=` (clap's `require_equals`), so
    // `--inspect 1.2.3.4:9229` treats the address as the positional argument.
    let parse_addr = |flag: &str, value: Option<&str>| match value {
      None => Ok(None),
      Some(v) => v.parse::<SocketAddr>().map(Some).map_err(|e| {
        anyhow::anyhow!("invalid value '{v}' for '{flag}=<HOST_AND_PORT>': {e}")
      }),
    };

    for arg in args {
      let (name, value) = match arg.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (arg.as_str(), None),
      };
      match name {
        "-h" | "--help" => {
          println!("{USAGE}");
          return Ok(None);
        }
        "--inspect" | "--inspect-brk" | "--inspect-wait" => {
          if let Some(previous) = seen_inspect_flag {
            bail!("the argument '{previous}' cannot be used with '{name}'");
          }
          seen_inspect_flag = Some(match name {
            "--inspect" => "--inspect",
            "--inspect-brk" => "--inspect-brk",
            _ => "--inspect-wait",
          });
          let addr = parse_addr(name, value)?;
          match name {
            "--inspect" => out.inspect = Some(addr),
            "--inspect-wait" => out.inspect_wait = Some(addr),
            // `--inspect-brk` is accepted but not wired up to the inspector
            // server, matching the previous clap-based behavior.
            _ => {}
          }
        }
        "--strace-ops" => out.strace_ops = true,
        "--strace-ops-summary" => out.strace_ops_summary = true,
        "--v8-flags" => {
          let Some(value) = value else {
            bail!("equal sign is needed when assigning values to '--v8-flags'");
          };
          out.v8_flags.extend(value.split(',').map(String::from));
        }
        _ if name.starts_with('-') && name != "-" => {
          bail!("unexpected argument '{name}' found");
        }
        _ => {
          if file_path.is_some() {
            bail!("unexpected argument '{arg}' found");
          }
          file_path = Some(arg);
        }
      }
    }

    let Some(file_path) = file_path else {
      bail!(
        "the following required arguments were not provided:\n  <file_path>"
      );
    };
    out.file_path = file_path;
    Ok(Some(out))
  }

  fn inspect(&self) -> Option<(SocketAddr, InspectMode)> {
    let default = || "127.0.0.1:9229".parse::<SocketAddr>().unwrap();
    if let Some(addr) = self.inspect {
      return Some((addr.unwrap_or_else(default), InspectMode::Immediate));
    }
    if let Some(addr) = self.inspect_wait {
      return Some((
        addr.unwrap_or_else(default),
        InspectMode::WaitForConnection,
      ));
    }
    None
  }
}

fn get_v8_flags_from_env() -> Vec<String> {
  std::env::var("DCORE_V8_FLAGS")
    .ok()
    .map(|flags| flags.split(',').map(String::from).collect::<Vec<String>>())
    .unwrap_or_default()
}

fn construct_v8_flags(
  v8_flags: &[String],
  env_v8_flags: Vec<String>,
) -> Vec<String> {
  std::iter::once("UNUSED_BUT_NECESSARY_ARG0".to_owned())
    .chain(env_v8_flags)
    .chain(v8_flags.iter().cloned())
    .collect::<Vec<_>>()
}

fn init_v8_flags(v8_flags: &[String]) {
  let env_v8_flags = get_v8_flags_from_env();
  if v8_flags.is_empty() && env_v8_flags.is_empty() {
    return;
  }

  let v8_flags_includes_help = env_v8_flags
    .iter()
    .chain(v8_flags)
    .any(|flag| flag == "-help" || flag == "--help");
  // Keep in sync with `standalone.rs`.
  let v8_flags = construct_v8_flags(v8_flags, env_v8_flags);
  let unrecognized_v8_flags = deno_core::v8_set_flags(v8_flags)
    .into_iter()
    .skip(1)
    .collect::<Vec<_>>();

  if !unrecognized_v8_flags.is_empty() {
    for f in unrecognized_v8_flags {
      eprintln!("error: V8 did not recognize flag '{f}'");
    }
    eprintln!("\nFor a list of V8 flags, use '--v8-flags=--help'");
    std::process::exit(1);
  }
  if v8_flags_includes_help {
    std::process::exit(0);
  }
}
