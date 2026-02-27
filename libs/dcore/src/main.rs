// Copyright 2018-2025 the Deno authors. MIT license.

use clap::ArgMatches;
use clap::builder::Arg;
use clap::builder::Command;
use deno_core::anyhow::Error;

use deno_core::RuntimeOptions;
use deno_core_testing::create_runtime_from_snapshot;

use std::net::SocketAddr;

use anyhow::Context;
use std::sync::Arc;

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

  let cli = build_cli();
  let mut matches = cli.get_matches();

  let file_path = matches.remove_one::<String>("file_path").unwrap();
  println!("Run {file_path}");

  let (maybe_inspector_addr, maybe_inspect_mode) =
    inspect_arg_parse(&mut matches).unzip();
  let inspector_server = if maybe_inspector_addr.is_some() {
    // TODO(bartlomieju): make it configurable
    let host = "127.0.0.1:9229".parse::<SocketAddr>().unwrap();
    Some(Arc::new(InspectorServer::new(host, "dcore")?))
  } else {
    None
  };

  let mut v8_flags = Vec::new();
  if let Some(flags) = matches.remove_many("v8-flags") {
    v8_flags = flags.collect();
  }

  init_v8_flags(&v8_flags);

  let (metrics_summary, mut js_runtime, _worker_host_side) =
    if matches.get_flag("strace-ops") || matches.get_flag("strace-ops-summary")
    {
      let (summary, op_metrics_factory_fn) = create_metrics(
        matches.get_flag("strace-ops"),
        matches.get_flag("strace-ops-summary"),
      );

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

  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()?;

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

fn build_cli() -> Command {
  Command::new("dcore")
    .arg(
      Arg::new("inspect")
        .long("inspect")
        .value_name("HOST_AND_PORT")
        .conflicts_with_all(["inspect-brk", "inspect-wait"])
        .help("Activate inspector on host:port (default: 127.0.0.1:9229)")
        .num_args(0..=1)
        .require_equals(true)
        .value_parser(clap::value_parser!(SocketAddr)),
    )
    .arg(
      Arg::new("inspect-brk")
        .long("inspect-brk")
        .conflicts_with_all(["inspect", "inspect-wait"])
        .value_name("HOST_AND_PORT")
        .help(
          "Activate inspector on host:port, wait for debugger to connect and break at the start of user script",
        )
        .num_args(0..=1)
        .require_equals(true)
        .value_parser(clap::value_parser!(SocketAddr)),
    )
    .arg(
      Arg::new("inspect-wait")
        .long("inspect-wait")
        .conflicts_with_all(["inspect", "inspect-brk"])
        .value_name("HOST_AND_PORT")
        .help(
          "Activate inspector on host:port and wait for debugger to connect before running user code",
        )
        .num_args(0..=1)
        .require_equals(true)
        .value_parser(clap::value_parser!(SocketAddr)),
    )
    .arg(
      Arg::new("file_path")
        .help("A relative or absolute file to a file to run")
        .value_hint(clap::ValueHint::FilePath)
        .value_parser(clap::value_parser!(String))
        .required(true),
    )
    .arg(
      Arg::new("strace-ops")
        .help("Output a trace of op execution on stderr")
        .long("strace-ops")
        .num_args(0)
        .required(false)
        .action(clap::ArgAction::SetTrue)

    ).arg(
      Arg::new("strace-ops-summary")
        .help("Output a summary of op execution on stderr when program exits")
        .long("strace-ops-summary")
        .action(clap::ArgAction::SetTrue)
    ).arg(
      Arg::new("v8-flags")
        .long("v8-flags")
        .num_args(..)
        .use_value_delimiter(true)
        .require_equals(true)
        .value_name("V8_FLAGS")
        .help("To see a list of all available flags use --v8-flags=--help
Flags can also be set via the DCORE_V8_FLAGS environment variable.
Any flags set with this flag are appended after the DCORE_V8_FLAGS environment variable")
    )
}

enum InspectMode {
  Immediate,
  WaitForConnection,
}

fn inspect_arg_parse(
  matches: &mut ArgMatches,
) -> Option<(SocketAddr, InspectMode)> {
  let default = || "127.0.0.1:9229".parse::<SocketAddr>().unwrap();
  if matches.contains_id("inspect") {
    let addr = matches
      .remove_one::<SocketAddr>("inspect")
      .unwrap_or_else(default);
    return Some((addr, InspectMode::Immediate));
  }
  if matches.contains_id("inspect-wait") {
    let addr = matches
      .remove_one::<SocketAddr>("inspect-wait")
      .unwrap_or_else(default);
    return Some((addr, InspectMode::WaitForConnection));
  }

  None
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
