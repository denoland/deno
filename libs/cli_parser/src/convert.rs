// Copyright 2018-2026 the Deno authors. MIT license.
#![allow(dead_code, reason = "conversion functions used by the deno CLI crate")]

//! Converts a raw `ParseResult` into the typed `Flags` struct.
//!
//! Each subcommand has a dedicated conversion function that extracts
//! the relevant parsed arguments and builds the corresponding
//! `DenoSubcommand` variant plus any shared `Flags` fields.

use std::net::IpAddr;
use std::net::SocketAddr;
use std::num::NonZeroU8;
use std::num::NonZeroU32;
use std::num::NonZeroUsize;

use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageKind;

use crate::error::CliError;
use crate::error::CliErrorKind;
use crate::flags::*;
use crate::types::ParseResult;

type UnstableFeatureEntry = (&'static str, Option<fn(&mut UnstableConfig)>);

/// Parse a socket address string like "127.0.0.1:9229" into SocketAddr.
fn parse_socket_addr(s: &str) -> Option<SocketAddr> {
  let default_port: u16 = 9229;
  // Try parsing as-is first (e.g. "127.0.0.1:9229")
  if let Ok(addr) = s.parse::<SocketAddr>() {
    return Some(addr);
  }
  // Try as just a port (e.g. "10000" or "0")
  if let Ok(port) = s.parse::<u16>() {
    return Some(SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port));
  }
  // Try as just an IP address without port (e.g. "192.168.0.1")
  if let Ok(ip) = s.parse::<IpAddr>() {
    return Some(SocketAddr::new(ip, default_port));
  }
  // Try as host:port where host might not be an IP
  if let Some((host, port_str)) = s.rsplit_once(':')
    && let Ok(port) = port_str.parse::<u16>()
  {
    if host.is_empty() {
      // Handle ":10000" format
      return Some(SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
      return Some(SocketAddr::new(ip, port));
    }
  }
  None
}

// ============================================================
// High-level API
// ============================================================

/// Parse command-line arguments and convert directly to `Flags`.
/// This is the main entry point matching Deno's `flags_from_vec`.
pub fn flags_from_vec(args: Vec<String>) -> Result<Flags, CliError> {
  let parsed = crate::parse::parse(&crate::defs::DENO_ROOT, &args)?;

  // If --version was set at the root level (not as a subcommand flag like
  // `upgrade --version <value>`), return DisplayVersion.
  // Check: version is present, no subcommand OR subcommand doesn't have its
  // own --version flag that takes a value.
  if parsed.get_bool("version") && parsed.get_one("version").is_none() {
    return Err(CliError::new(CliErrorKind::DisplayVersion, "version"));
  }

  // If --help was set, generate help text
  if parsed.contains("help") {
    let root = &crate::defs::DENO_ROOT;
    let cmd = if let Some(ref sub) = parsed.subcommand {
      root.find_subcommand(sub).unwrap_or(root)
    } else {
      root
    };
    let help_text = crate::help::render_help(cmd);
    let mut flags = Flags::default();
    global_args_parse(&parsed, &mut flags);
    flags.subcommand = DenoSubcommand::Help(HelpFlags { help: help_text });
    return Ok(flags);
  }

  let mut flags = convert(parsed)?;
  apply_node_options(&mut flags);
  Ok(flags)
}

// ============================================================
// Main entry point
// ============================================================

/// Convert a raw `ParseResult` into a fully typed `Flags`.
pub fn convert(result: ParseResult) -> Result<Flags, CliError> {
  let mut flags = Flags::default();

  // Global flags (log-level, quiet)
  global_args_parse(&result, &mut flags);

  // Unstable feature flags (applies to all subcommands)
  unstable_args_parse(&result, &mut flags);

  match result.subcommand.as_deref() {
    Some("run") => run_parse(&result, &mut flags),
    Some("serve") => serve_parse(&result, &mut flags),
    Some("eval") => eval_parse(&result, &mut flags),
    Some("fmt") => fmt_parse(&result, &mut flags),
    Some("lint") => lint_parse(&result, &mut flags),
    Some("test") => test_parse(&result, &mut flags),
    Some("upgrade") => upgrade_parse(&result, &mut flags)?,
    Some("cache") => cache_parse(&result, &mut flags),
    Some("check") => check_parse(&result, &mut flags),
    Some("info") => info_parse(&result, &mut flags),
    Some("doc") => doc_parse(&result, &mut flags),
    Some("task") => task_parse(&result, &mut flags),
    Some("bench") => bench_parse(&result, &mut flags),
    Some("compile") => compile_parse(&result, &mut flags),
    Some("coverage") => coverage_parse(&result, &mut flags),
    Some("repl") => repl_parse(&result, &mut flags, false),
    Some("install" | "i") => install_parse(&result, &mut flags)?,
    Some("uninstall") => uninstall_parse(&result, &mut flags),
    Some("completions") => completions_parse(&result, &mut flags),
    Some("init") => init_parse(&result, &mut flags),
    Some("create") => create_parse(&result, &mut flags),
    Some("jupyter") => jupyter_parse(&result, &mut flags),
    Some("publish") => publish_parse(&result, &mut flags),
    Some("add") => add_parse(&result, &mut flags),
    Some("remove" | "rm") => remove_parse(&result, &mut flags),
    Some("outdated") => outdated_parse(&result, &mut flags, false),
    Some("update") => outdated_parse(&result, &mut flags, true),
    Some("clean") => clean_parse(&result, &mut flags),
    Some("approve-scripts" | "approve-builds") => {
      approve_scripts_parse(&result, &mut flags)
    }
    Some("types") => types_parse(&mut flags),
    Some("lsp") => lsp_parse(&mut flags),
    Some("vendor") => vendor_parse(&mut flags),
    Some("deploy") => deploy_parse(&result, &mut flags, false),
    Some("sandbox") => deploy_parse(&result, &mut flags, true),
    Some("bundle") => bundle_parse(&result, &mut flags),
    Some("audit") => audit_parse(&result, &mut flags),
    Some("x") => x_parse(&result, &mut flags),
    Some("json_reference") => json_reference_parse(&mut flags),
    Some("help") => help_subcommand_parse(&result, &mut flags),
    None => default_parse(&result, &mut flags)?,
    Some(other) => {
      return Err(CliError::new(
        CliErrorKind::InvalidValue,
        format!("Unknown subcommand: {other}"),
      ));
    }
  }

  // Validate --no-clear-screen requires --watch
  no_clear_screen_requires_watch(&result)?;

  // Validate permission args
  validate_permission_args(&result, &flags)?;

  // Trailing args go into flags.argv (unless already handled by the subcommand)
  let handled_trailing = matches!(
    result.subcommand.as_deref(),
    Some("create")
      | Some("init")
      | Some("compile")
      | Some("install")
      | Some("i")
  );
  if !result.trailing.is_empty() && !handled_trailing {
    flags.argv.extend(result.trailing.iter().cloned());
  }

  Ok(flags)
}

// ============================================================
// Default (no subcommand)
// ============================================================

/// When no subcommand is provided: if there is a script arg, treat as
/// bare `run`; otherwise enter REPL.
fn default_parse(
  result: &ParseResult,
  flags: &mut Flags,
) -> Result<(), CliError> {
  // Check if there's a script_arg (from default subcommand = run)
  let script = result.get_one("script_arg");
  if let Some(script) = script {
    // Check if the script name is actually a known subcommand name.
    // If so, a non-global flag was placed before it and we should error.
    let known_subcommands = [
      "run",
      "serve",
      "eval",
      "fmt",
      "lint",
      "test",
      "upgrade",
      "cache",
      "check",
      "info",
      "doc",
      "task",
      "bench",
      "compile",
      "coverage",
      "repl",
      "install",
      "uninstall",
      "completions",
      "init",
      "create",
      "jupyter",
      "publish",
      "add",
      "remove",
      "outdated",
      "update",
      "clean",
      "approve-scripts",
      "types",
      "lsp",
      "vendor",
      "deploy",
      "sandbox",
      "bundle",
      "audit",
      "x",
    ];
    if known_subcommands.contains(&script) {
      // Check if any non-global flags were parsed before this
      // subcommand name. If so, the user likely put flags before
      // the subcommand.
      let has_non_global_flags = result.args.iter().any(|a| {
        a.is_present
          && a.name != "script_arg"
          && a.name != "log-level"
          && a.name != "quiet"
          && a.name != "unstable"
      });
      if has_non_global_flags {
        // Find which flags were set
        let flag_names: Vec<String> = result
          .args
          .iter()
          .filter(|a| {
            a.is_present
              && a.name != "script_arg"
              && a.name != "log-level"
              && a.name != "quiet"
              && a.name != "unstable"
          })
          .filter_map(|a| {
            // Try to find the long flag name
            let root = &crate::defs::DENO_ROOT;
            let cmd = root.find_subcommand("run").unwrap_or(root);
            cmd
              .all_args()
              .find(|ad| ad.name == a.name)
              .and_then(|ad| ad.long)
              .map(|l| format!("--{l}"))
          })
          .collect();
        let first_flag = flag_names
          .first()
          .cloned()
          .unwrap_or_else(|| "unknown".to_string());
        return Err(
          CliError::new(
            CliErrorKind::UnknownFlag,
            format!("unexpected argument '{first_flag}' found"),
          )
          .with_suggestion(format!("'{script} {first_flag}' exists")),
        );
      }
    }

    // Bare run — same as `deno run` but with bare=true
    runtime_args_parse(result, flags, true, true, true);
    ext_arg_parse(result, flags);
    flags.tunnel = result.get_bool("tunnel");
    flags.code_cache_enabled = !result.get_bool("no-code-cache");
    let coverage_dir = if result.contains("coverage") {
      Some(result.get_one("coverage").unwrap_or("coverage").to_string())
    } else {
      None
    };
    cpu_prof_parse(result, flags);

    flags.subcommand = DenoSubcommand::Run(RunFlags {
      script: script.to_string(),
      watch: watch_arg_parse_with_paths(result),
      bare: true,
      coverage_dir,
      print_task_list: false,
    });
  } else {
    // Check if any run-specific flags were provided without a script.
    // If so, the user likely meant to run a script but forgot it.
    let has_v8_help = result
      .get_many("v8-flags")
      .is_some_and(|v| v.iter().any(|f| f == "--help"));

    // Check for flags that indicate run intent (not REPL)
    let has_run_flags = result.get_bool("no-config")
      || result.contains("config")
      || result.contains("import-map")
      || result.contains("no-remote")
      || result.contains("no-npm")
      || result.contains("lock")
      || result.contains("no-lock")
      || result.contains("reload")
      || result.contains("frozen-lockfile")
      || result.contains("cert")
      || result.contains("check")
      || result.contains("no-check");

    if has_run_flags && !has_v8_help {
      return Err(CliError::new(
        CliErrorKind::MissingRequired,
        "[SCRIPT_ARG] may only be omitted with --v8-flags=--help, else to use the repl with arguments, please use the `deno repl` subcommand\n\nUsage: deno [OPTIONS] [COMMAND] [SCRIPT_ARG]...",
      ));
    }

    // No script → REPL with default command
    repl_parse(result, flags, true);
  }
  Ok(())
}

// ============================================================
// Global args
// ============================================================

fn global_args_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(level) = result.get_one("log-level") {
    flags.log_level = match level {
      "trace" => Some(log::Level::Trace),
      "debug" => Some(log::Level::Debug),
      "info" => Some(log::Level::Info),
      "warn" => Some(log::Level::Warn),
      "error" => Some(log::Level::Error),
      _ => None,
    };
  }
  if result.get_bool("quiet") {
    flags.log_level = Some(log::Level::Error);
  }
}

// ============================================================
// Shared arg group parsers
// ============================================================

fn compile_args_parse(result: &ParseResult, flags: &mut Flags) {
  compile_args_without_check_parse(result, flags);
  no_check_arg_parse(result, flags);
  check_arg_parse(result, flags);
}

fn compile_args_without_check_parse(result: &ParseResult, flags: &mut Flags) {
  import_map_arg_parse(result, flags);
  no_remote_arg_parse(result, flags);
  no_npm_arg_parse(result, flags);
  node_modules_and_vendor_dir_arg_parse(result, flags);
  node_conditions_args_parse(result, flags);
  config_args_parse(result, flags);
  reload_arg_parse(result, flags);
  lock_args_parse(result, flags);
  ca_file_arg_parse(result, flags);
  unsafely_ignore_certificate_errors_parse(result, flags);
  min_dep_age_arg_parse(result, flags);
}

fn runtime_args_parse(
  result: &ParseResult,
  flags: &mut Flags,
  include_perms: bool,
  include_inspector: bool,
  include_allow_scripts: bool,
) {
  compile_args_parse(result, flags);
  cached_only_arg_parse(result, flags);
  if include_perms {
    permission_args_parse(result, flags);
  }
  if include_inspector {
    inspect_arg_parse(result, flags);
  }
  if include_allow_scripts {
    allow_scripts_arg_parse(result, flags);
  }
  location_arg_parse(result, flags);
  v8_flags_arg_parse(result, flags);
  seed_arg_parse(result, flags);
  enable_testing_features_arg_parse(result, flags);
  env_file_arg_parse(result, flags);
  trace_ops_parse(result, flags);
  eszip_arg_parse(result, flags);
  preload_arg_parse(result, flags);
  require_arg_parse(result, flags);
}

/// Expand bare port entries (e.g. ":8080") into full host:port entries
/// for 0.0.0.0, 127.0.0.1, and localhost.
fn expand_net_list(entries: Vec<String>) -> Vec<String> {
  let mut out = Vec::new();
  for entry in entries {
    if let Some(port_str) = entry.strip_prefix(':')
      && port_str.parse::<u16>().is_ok()
    {
      for host in &["0.0.0.0", "127.0.0.1", "localhost"] {
        out.push(format!("{}:{}", host, port_str));
      }
      continue;
    }
    out.push(entry);
  }
  out
}

fn permission_args_parse(result: &ParseResult, flags: &mut Flags) {
  if result.contains("permission-set") {
    flags.permission_set =
      Some(result.get_one("permission-set").unwrap_or("").to_string());
  }

  // Helper: for permission flags that use Append + ZeroOrMore + value_delimiter
  // get_many returns Some(&[]) when flag is present without value,
  // Some(&[values...]) when flag has values, None when absent.
  macro_rules! perm_flag {
    ($name:expr, $field:expr) => {
      if let Some(values) = result.get_many($name) {
        $field = Some(values.iter().map(|s| s.to_string()).collect());
      }
    };
  }

  perm_flag!("allow-read", flags.permissions.allow_read);
  perm_flag!("deny-read", flags.permissions.deny_read);
  perm_flag!("ignore-read", flags.permissions.ignore_read);
  perm_flag!("allow-write", flags.permissions.allow_write);
  perm_flag!("deny-write", flags.permissions.deny_write);
  perm_flag!("allow-net", flags.permissions.allow_net);
  perm_flag!("deny-net", flags.permissions.deny_net);

  // Expand bare port entries in net lists
  if let Some(ref mut net) = flags.permissions.allow_net {
    *net = expand_net_list(std::mem::take(net));
  }
  if let Some(ref mut net) = flags.permissions.deny_net {
    *net = expand_net_list(std::mem::take(net));
  }
  perm_flag!("allow-env", flags.permissions.allow_env);
  perm_flag!("deny-env", flags.permissions.deny_env);
  perm_flag!("ignore-env", flags.permissions.ignore_env);
  perm_flag!("allow-run", flags.permissions.allow_run);
  perm_flag!("deny-run", flags.permissions.deny_run);
  perm_flag!("allow-sys", flags.permissions.allow_sys);
  perm_flag!("deny-sys", flags.permissions.deny_sys);
  perm_flag!("allow-ffi", flags.permissions.allow_ffi);
  perm_flag!("deny-ffi", flags.permissions.deny_ffi);
  perm_flag!("allow-import", flags.permissions.allow_import);
  perm_flag!("deny-import", flags.permissions.deny_import);

  if result.get_bool("allow-all") {
    flags.permissions.allow_all = true;
  }

  if result.get_bool("no-prompt") {
    flags.permissions.no_prompt = true;
  }
}

fn inspect_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.contains("inspect") {
    let val = result.get_one("inspect").unwrap_or("127.0.0.1:9229");
    flags.inspect = parse_socket_addr(val);
  }
  if result.contains("inspect-brk") {
    let val = result.get_one("inspect-brk").unwrap_or("127.0.0.1:9229");
    flags.inspect_brk = parse_socket_addr(val);
  }
  if result.contains("inspect-wait") {
    let val = result.get_one("inspect-wait").unwrap_or("127.0.0.1:9229");
    flags.inspect_wait = parse_socket_addr(val);
  }
  if let Some(uid_str) = result.get_one("inspect-publish-uid") {
    let mut uid = InspectPublishUid {
      console: false,
      http: false,
    };
    for part in uid_str.split(',') {
      match part.trim() {
        "stderr" | "console" => uid.console = true,
        "http" => uid.http = true,
        _ => {}
      }
    }
    flags.inspect_publish_uid = Some(uid);
  }
}

fn config_args_parse(result: &ParseResult, flags: &mut Flags) {
  flags.config_flag = if result.get_bool("no-config") {
    ConfigFlag::Disabled
  } else if let Some(config) = result.get_one("config") {
    ConfigFlag::Path(config.to_string())
  } else {
    ConfigFlag::Discover
  };
}

fn import_map_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(path) = result.get_one("import-map") {
    flags.import_map_path = Some(path.to_string());
  }
}

fn no_remote_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.get_bool("no-remote") {
    flags.no_remote = true;
  }
}

fn no_npm_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.get_bool("no-npm") {
    flags.no_npm = true;
  }
}

fn node_modules_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.contains("node-modules-dir") {
    let value = result.get_one("node-modules-dir");
    flags.node_modules_dir = Some(match value {
      Some("auto" | "true") | None => NodeModulesDirMode::Auto,
      Some("manual") => NodeModulesDirMode::Manual,
      Some("none" | "false") => NodeModulesDirMode::None,
      Some(_) => NodeModulesDirMode::Auto,
    });
  }
}

fn vendor_dir_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.contains("vendor") {
    let value = result.get_one("vendor");
    flags.vendor = Some(!matches!(value, Some("false")));
  }
}

fn node_modules_and_vendor_dir_arg_parse(
  result: &ParseResult,
  flags: &mut Flags,
) {
  node_modules_arg_parse(result, flags);
  vendor_dir_arg_parse(result, flags);
}

fn node_conditions_args_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(values) = result.get_many("node-conditions") {
    flags.node_conditions = values.iter().map(|s| s.to_string()).collect();
  }
}

fn reload_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(values) = result.get_many("reload") {
    if values.is_empty() {
      flags.reload = true;
    } else {
      flags.cache_blocklist = values.iter().map(|s| s.to_string()).collect();
      flags.reload = false;
    }
  }
}

fn lock_args_parse(result: &ParseResult, flags: &mut Flags) {
  if result.contains("lock") {
    flags.lock =
      Some(result.get_one("lock").unwrap_or("deno.lock").to_string());
  }
  if result.get_bool("no-lock") {
    flags.no_lock = true;
  }
  frozen_lockfile_arg_parse(result, flags);
}

fn frozen_lockfile_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.contains("frozen-lockfile") {
    let value = result.get_one("frozen-lockfile");
    flags.frozen_lockfile = Some(match value {
      Some("false") => false,
      _ => true, // --frozen-lockfile without value or with any other value
    });
  }
}

fn ca_file_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(cert) = result.get_one("cert") {
    if let Some(b64) = cert.strip_prefix("base64:") {
      if let Some(bytes) = base64_decode(b64) {
        flags.ca_data = Some(CaData::Bytes(bytes));
      } else {
        flags.ca_data = Some(CaData::File(cert.to_string()));
      }
    } else {
      flags.ca_data = Some(CaData::File(cert.to_string()));
    }
  }
}

/// Simple base64 decoder (standard alphabet, with optional padding).
fn base64_decode(input: &str) -> Option<Vec<u8>> {
  const TABLE: [u8; 128] = {
    let mut t = [255u8; 128];
    let mut i = 0u8;
    while i < 26 {
      t[(b'A' + i) as usize] = i;
      t[(b'a' + i) as usize] = i + 26;
      i += 1;
    }
    let mut i = 0u8;
    while i < 10 {
      t[(b'0' + i) as usize] = i + 52;
      i += 1;
    }
    t[b'+' as usize] = 62;
    t[b'/' as usize] = 63;
    t
  };

  let input = input.trim_end_matches('=');
  let mut out = Vec::with_capacity(input.len() * 3 / 4);
  let mut buf: u32 = 0;
  let mut bits: u32 = 0;

  for &b in input.as_bytes() {
    if b >= 128 {
      return None;
    }
    let val = TABLE[b as usize];
    if val == 255 {
      return None;
    }
    buf = (buf << 6) | val as u32;
    bits += 6;
    if bits >= 8 {
      bits -= 8;
      out.push((buf >> bits) as u8);
      buf &= (1 << bits) - 1;
    }
  }

  Some(out)
}

fn unsafely_ignore_certificate_errors_parse(
  result: &ParseResult,
  flags: &mut Flags,
) {
  if let Some(values) = result.get_many("unsafely-ignore-certificate-errors") {
    flags.unsafely_ignore_certificate_errors =
      Some(values.iter().map(|s| s.to_string()).collect());
  }
}

fn min_dep_age_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(age) = result.get_one("min-dep-age") {
    flags.minimum_dependency_age = deno_config::parse_minutes_duration_or_date(
      &sys_traits::impls::RealSys,
      age,
    )
    .ok();
  }
}

fn cached_only_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.get_bool("cached-only") {
    flags.cached_only = true;
  }
}

fn location_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(loc) = result.get_one("location") {
    flags.location = deno_core::url::Url::parse(loc).ok();
  }
}

fn v8_flags_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(values) = result.get_many("v8-flags") {
    flags.v8_flags = values.iter().map(|s| s.to_string()).collect();
  }
}

fn seed_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(seed_str) = result.get_one("seed")
    && let Ok(seed) = seed_str.parse::<u64>()
  {
    flags.seed = Some(seed);
    flags.v8_flags.push(format!("--random-seed={seed}"));
  }
}

fn enable_testing_features_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.get_bool("enable-testing-features") {
    flags.enable_testing_features = true;
  }
}

fn env_file_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.contains("env-file") {
    // Get the parsed arg directly to check count vs values
    let parsed_arg = result.args.iter().find(|a| a.name == "env-file");
    if let Some(pa) = parsed_arg {
      let mut files = Vec::new();
      // Each occurrence where count > values.len() was a bare --env-file
      // (no value), which defaults to ".env"
      let bare_count = pa.count.saturating_sub(pa.values.len());
      for _ in 0..bare_count {
        files.push(".env".to_string());
      }
      files.extend(pa.values.iter().cloned());
      if files.is_empty() {
        files.push(".env".to_string());
      }
      flags.env_file = Some(files);
    } else {
      flags.env_file = Some(vec![".env".to_string()]);
    }
  }
}

fn trace_ops_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(values) = result.get_many("trace-ops") {
    flags.trace_ops = Some(values.iter().map(|s| s.to_string()).collect());
  }
}

fn eszip_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.get_bool("eszip-internal-do-not-use") {
    flags.eszip = true;
  }
}

fn preload_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(values) = result.get_many("preload") {
    flags.preload = values.iter().map(|s| s.to_string()).collect();
  }
}

fn require_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(values) = result.get_many("require") {
    flags.require = values.iter().map(|s| s.to_string()).collect();
  }
}

fn ext_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(ext) = result.get_one("ext") {
    flags.ext = Some(ext.to_string());
  }
}

fn unstable_args_parse(result: &ParseResult, flags: &mut Flags) {
  if result.get_bool("unstable") {
    flags.unstable_config.legacy_flag_enabled = true;
  }

  // Map each unstable feature flag to features list and individual bools
  let unstable_features: &[UnstableFeatureEntry] = &[
    (
      "unstable-bare-node-builtins",
      Some(|c: &mut UnstableConfig| c.bare_node_builtins = true),
    ),
    (
      "unstable-detect-cjs",
      Some(|c: &mut UnstableConfig| c.detect_cjs = true),
    ),
    (
      "unstable-lazy-dynamic-imports",
      Some(|c: &mut UnstableConfig| c.lazy_dynamic_imports = true),
    ),
    (
      "unstable-sloppy-imports",
      Some(|c: &mut UnstableConfig| c.sloppy_imports = true),
    ),
    (
      "unstable-npm-lazy-caching",
      Some(|c: &mut UnstableConfig| c.npm_lazy_caching = true),
    ),
    (
      "unstable-raw-imports",
      Some(|c: &mut UnstableConfig| c.raw_imports = true),
    ),
    (
      "unstable-tsgo",
      Some(|c: &mut UnstableConfig| c.tsgo = true),
    ),
    ("unstable-ffi", None),
    ("unstable-worker-options", None),
    ("unstable-broadcast-channel", None),
    ("unstable-bundle", None),
    ("unstable-byonm", None),
    ("unstable-cron", None),
    ("unstable-fs", None),
    ("unstable-http", None),
    ("unstable-kv", None),
    ("unstable-lockfile-v5", None),
    ("unstable-net", None),
    ("unstable-no-legacy-abort", None),
    ("unstable-node-globals", None),
    ("unstable-otel", None),
    ("unstable-process", None),
    ("unstable-subdomain-wildcards", None),
    ("unstable-temporal", None),
    ("unstable-unsafe-proto", None),
    ("unstable-vsock", None),
    ("unstable-webgpu", None),
  ];

  for (flag_name, setter) in unstable_features {
    if result.get_bool(flag_name) {
      // Extract feature name (strip "unstable-" prefix)
      let feature = flag_name.strip_prefix("unstable-").unwrap_or(flag_name);
      flags.unstable_config.features.push(feature.to_string());
      if let Some(setter) = setter {
        setter(&mut flags.unstable_config);
      }
    }
  }

  // Sort features for deterministic output
  flags.unstable_config.features.sort();
}

fn allow_scripts_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(values) = result.get_many("allow-scripts") {
    if values.is_empty() {
      flags.allow_scripts = PackagesAllowedScripts::All;
    } else {
      flags.allow_scripts = PackagesAllowedScripts::Some(
        values
          .iter()
          .filter_map(|s| {
            let dep = JsrDepPackageReq::from_str_loose(s).ok()?;
            if dep.kind != PackageKind::Npm {
              return None;
            }
            Some(dep.req)
          })
          .collect(),
      );
    }
  }
}

fn no_check_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.contains("no-check") {
    if let Some(value) = result.get_one("no-check") {
      match value {
        "remote" => flags.type_check_mode = TypeCheckMode::Local,
        _ => {
          // Invalid value for --no-check — keep default
        }
      }
    } else {
      flags.type_check_mode = TypeCheckMode::None;
    }
  }
}

fn check_arg_parse(result: &ParseResult, flags: &mut Flags) {
  if result.contains("check") {
    if let Some(value) = result.get_one("check") {
      match value {
        "all" => flags.type_check_mode = TypeCheckMode::All,
        _ => {
          // Invalid value for --check — keep default (no change)
        }
      }
    } else {
      flags.type_check_mode = TypeCheckMode::Local;
    }
  }
}

fn cpu_prof_parse(result: &ParseResult, flags: &mut Flags) {
  let enabled = result.get_bool("cpu-prof");
  let dir = result.get_one("cpu-prof-dir").map(|s| s.to_string());
  let name = result.get_one("cpu-prof-name").map(|s| s.to_string());
  let interval = result
    .get_one("cpu-prof-interval")
    .and_then(|s| s.parse::<u32>().ok());
  // md and flamegraph flags do not exist in our defs currently,
  // but handle them gracefully
  let md = result.get_bool("cpu-prof-md");
  let flamegraph = result.get_bool("cpu-prof-flamegraph");

  if enabled
    || dir.is_some()
    || name.is_some()
    || interval.is_some()
    || md
    || flamegraph
  {
    flags.cpu_prof = Some(CpuProfFlags {
      dir,
      name,
      interval,
      md,
      flamegraph,
    });
  }
}

fn allow_and_deny_import_parse(result: &ParseResult, flags: &mut Flags) {
  if let Some(values) = result.get_many("allow-import") {
    flags.permissions.allow_import =
      Some(values.iter().map(|s| s.to_string()).collect());
  }
  if let Some(values) = result.get_many("deny-import") {
    flags.permissions.deny_import =
      Some(values.iter().map(|s| s.to_string()).collect());
  }
}

// ============================================================
// Watch helpers
// ============================================================

/// Parse watch flags without paths (for fmt, lint, bench).
fn watch_arg_parse(result: &ParseResult) -> Option<WatchFlags> {
  if result.contains("watch") {
    Some(WatchFlags {
      hmr: false,
      no_clear_screen: result.get_bool("no-clear-screen"),
      exclude: result
        .get_many("watch-exclude")
        .map(|v| v.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default(),
    })
  } else {
    None
  }
}

/// Parse watch flags with paths (for run, serve, test).
fn watch_arg_parse_with_paths(
  result: &ParseResult,
) -> Option<WatchFlagsWithPaths> {
  if result.contains("watch") {
    let paths = result
      .get_many("watch")
      .map(|v| v.iter().map(|s| s.to_string()).collect())
      .unwrap_or_default();
    return Some(WatchFlagsWithPaths {
      paths,
      hmr: false,
      no_clear_screen: result.get_bool("no-clear-screen"),
      exclude: result
        .get_many("watch-exclude")
        .map(|v| v.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default(),
    });
  }

  if result.contains("hmr") {
    let paths = result
      .get_many("hmr")
      .map(|v| v.iter().map(|s| s.to_string()).collect())
      .unwrap_or_default();
    return Some(WatchFlagsWithPaths {
      paths,
      hmr: true,
      no_clear_screen: result.get_bool("no-clear-screen"),
      exclude: result
        .get_many("watch-exclude")
        .map(|v| v.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default(),
    });
  }

  None
}

// ============================================================
// Subcommand conversion functions
// ============================================================

fn no_clear_screen_requires_watch(
  result: &ParseResult,
) -> Result<(), CliError> {
  if result.get_bool("no-clear-screen")
    && !result.contains("watch")
    && !result.contains("hmr")
  {
    return Err(CliError::new(
      CliErrorKind::MissingRequired,
      "the following required arguments were not provided:\n  --watch\n\n  tip: '--no-clear-screen' requires '--watch' to be provided",
    ));
  }
  Ok(())
}

/// Known valid sys descriptors.
const VALID_SYS_DESCRIPTORS: &[&str] = &[
  "hostname",
  "osRelease",
  "osUptime",
  "loadavg",
  "networkInterfaces",
  "systemMemoryInfo",
  "uid",
  "gid",
  "cpus",
  "homedir",
  "getegid",
  "username",
  "statfs",
  "getPriority",
  "setPriority",
  "userInfo",
];

fn validate_permission_args(
  _result: &ParseResult,
  flags: &Flags,
) -> Result<(), CliError> {
  // Validate env var names: reject names containing '=' or '\0'
  if let Some(ref envs) = flags.permissions.allow_env {
    for name in envs {
      if name.contains('=') || name.contains('\0') {
        return Err(CliError::new(
          CliErrorKind::InvalidValue,
          format!("invalid env var name: '{name}'"),
        ));
      }
    }
  }
  if let Some(ref envs) = flags.permissions.deny_env {
    for name in envs {
      if name.contains('=') || name.contains('\0') {
        return Err(CliError::new(
          CliErrorKind::InvalidValue,
          format!("invalid env var name: '{name}'"),
        ));
      }
    }
  }

  // Validate sys descriptor names
  if let Some(ref sys) = flags.permissions.allow_sys {
    for name in sys {
      if !name.is_empty() && !VALID_SYS_DESCRIPTORS.contains(&name.as_str()) {
        return Err(CliError::new(
          CliErrorKind::InvalidValue,
          format!("unknown sys descriptor: '{name}'"),
        ));
      }
    }
  }
  if let Some(ref sys) = flags.permissions.deny_sys {
    for name in sys {
      if !name.is_empty() && !VALID_SYS_DESCRIPTORS.contains(&name.as_str()) {
        return Err(CliError::new(
          CliErrorKind::InvalidValue,
          format!("unknown sys descriptor: '{name}'"),
        ));
      }
    }
  }

  // Validate reload values are valid URLs or specifier prefixes (npm:, jsr:)
  for url in &flags.cache_blocklist {
    if url.is_empty()
      || url.starts_with("./")
      || url.starts_with('/')
    {
      return Err(CliError::new(
        CliErrorKind::InvalidValue,
        format!("invalid reload URL: '{url}'"),
      ));
    }
    // Must contain "://" (http://, https://, file://) or be a known
    // specifier prefix (npm:, jsr:)
    if !url.contains("://") && !url.starts_with("npm:") && !url.starts_with("jsr:") {
      return Err(CliError::new(
        CliErrorKind::InvalidValue,
        format!("invalid reload URL: '{url}'"),
      ));
    }
  }

  // Validate net/import flags don't contain URLs (must be domains/IPs only)
  fn check_no_url_scheme(
    values: &Option<Vec<String>>,
    _flag_name: &str,
  ) -> Result<(), CliError> {
    if let Some(vals) = values {
      for val in vals {
        if val.contains("://") {
          return Err(CliError::new(
            CliErrorKind::InvalidValue,
            format!(
              "invalid value '{val}': URLs are not supported, only domains and ips"
            ),
          ));
        }
      }
    }
    Ok(())
  }
  check_no_url_scheme(&flags.permissions.allow_net, "--allow-net")?;
  check_no_url_scheme(&flags.permissions.deny_net, "--deny-net")?;
  check_no_url_scheme(&flags.permissions.allow_import, "--allow-import")?;
  check_no_url_scheme(&flags.permissions.deny_import, "--deny-import")?;

  // Validate --allow-all conflicts with specific allow flags
  if flags.permissions.allow_all {
    let conflicting_flags: &[(&str, &Option<Vec<String>>)] = &[
      ("--allow-read", &flags.permissions.allow_read),
      ("--allow-write", &flags.permissions.allow_write),
      ("--allow-net", &flags.permissions.allow_net),
      ("--allow-env", &flags.permissions.allow_env),
      ("--allow-run", &flags.permissions.allow_run),
      ("--allow-sys", &flags.permissions.allow_sys),
      ("--allow-ffi", &flags.permissions.allow_ffi),
      ("--allow-import", &flags.permissions.allow_import),
    ];

    // Only check if this is not the REPL default (where allow_all is auto-set)
    if !matches!(
      flags.subcommand,
      DenoSubcommand::Repl(ReplFlags {
        is_default_command: true,
        ..
      })
    ) {
      for (flag_name, value) in conflicting_flags {
        if value.is_some() {
          return Err(CliError::new(
            CliErrorKind::InvalidValue,
            format!("--allow-all conflicts with {flag_name}"),
          ));
        }
      }
    }
  }

  Ok(())
}

/// Split a comma-separated value string, where `,,` is an escaped literal comma.
/// Returns an error if any empty values are produced.
#[cfg(test)]
pub fn escape_and_split_commas(s: String) -> Result<Vec<String>, CliError> {
  let mut result = vec![];
  let mut current = String::new();
  let mut chars = s.chars();

  while let Some(c) = chars.next() {
    if c == ',' {
      if let Some(next) = chars.next() {
        if next == ',' {
          current.push(',');
        } else {
          if current.is_empty() {
            return Err(CliError::new(
              CliErrorKind::InvalidValue,
              "Empty values are not allowed",
            ));
          }

          result.push(current.clone());
          current.clear();
          current.push(next);
        }
      } else {
        return Err(CliError::new(
          CliErrorKind::InvalidValue,
          "Empty values are not allowed",
        ));
      }
    } else {
      current.push(c);
    }
  }

  if current.is_empty() {
    return Err(CliError::new(
      CliErrorKind::InvalidValue,
      "Empty values are not allowed",
    ));
  }

  result.push(current);

  Ok(result)
}

fn run_parse(result: &ParseResult, flags: &mut Flags) {
  runtime_args_parse(result, flags, true, true, true);
  ext_arg_parse(result, flags);

  flags.tunnel = result.get_bool("tunnel");
  flags.code_cache_enabled = !result.get_bool("no-code-cache");
  let coverage_dir = if result.contains("coverage") {
    Some(result.get_one("coverage").unwrap_or("coverage").to_string())
  } else {
    None
  };
  cpu_prof_parse(result, flags);

  if let Some(script) = result.get_one("script_arg") {
    flags.subcommand = DenoSubcommand::Run(RunFlags {
      script: script.to_string(),
      watch: watch_arg_parse_with_paths(result),
      bare: false,
      coverage_dir,
      print_task_list: false,
    });
  } else if !flags.v8_flags.is_empty() {
    // `deno run --v8-flags=--help` with no script
    flags.subcommand = DenoSubcommand::Run(RunFlags {
      script: "_".to_string(),
      watch: None,
      bare: false,
      coverage_dir: None,
      print_task_list: false,
    });
  } else {
    // `deno run` with no script — show available tasks
    flags.subcommand = DenoSubcommand::Run(RunFlags {
      script: String::new(),
      watch: None,
      bare: false,
      coverage_dir: None,
      print_task_list: true,
    });
  }
}

fn serve_parse(result: &ParseResult, flags: &mut Flags) {
  let port = result
    .get_one("port")
    .and_then(|s| s.parse::<u16>().ok())
    .unwrap_or(8000);
  let host = result
    .get_one("host")
    .map(|s| s.to_string())
    .unwrap_or_else(|| "0.0.0.0".to_string());

  runtime_args_parse(result, flags, true, true, true);
  flags.code_cache_enabled = !result.get_bool("no-code-cache");
  flags.tunnel = result.get_bool("tunnel");

  let script = result
    .get_one("script_arg")
    .map(|s| s.to_string())
    .unwrap_or_default();

  ext_arg_parse(result, flags);
  cpu_prof_parse(result, flags);

  let parallel = result.get_bool("parallel");

  flags.subcommand = DenoSubcommand::Serve(ServeFlags {
    script,
    watch: watch_arg_parse_with_paths(result),
    port,
    host,
    parallel,
    open_site: false,
  });
}

fn eval_parse(result: &ParseResult, flags: &mut Flags) {
  runtime_args_parse(result, flags, false, true, false);
  // eval implies allow all permissions
  flags.permissions.allow_all = true;

  ext_arg_parse(result, flags);
  cpu_prof_parse(result, flags);

  let print = result.get_bool("print");
  let code = result
    .get_one("code_arg")
    .map(|s| s.to_string())
    .unwrap_or_default();

  flags.subcommand = DenoSubcommand::Eval(EvalFlags { print, code });
}

fn fmt_parse(result: &ParseResult, flags: &mut Flags) {
  config_args_parse(result, flags);
  ext_arg_parse(result, flags);

  let include = result
    .get_many("files")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();
  let ignore = result
    .get_many("ignore")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();

  let use_tabs = if result.contains("use-tabs") {
    Some(!matches!(result.get_one("use-tabs"), Some("false")))
  } else {
    None
  };

  let line_width = result
    .get_one("line-width")
    .and_then(|s| s.parse::<u32>().ok())
    .and_then(NonZeroU32::new);
  let indent_width = result
    .get_one("indent-width")
    .and_then(|s| s.parse::<u8>().ok())
    .and_then(NonZeroU8::new);

  let single_quote = if result.contains("single-quote") {
    Some(!matches!(result.get_one("single-quote"), Some("false")))
  } else {
    None
  };

  let prose_wrap = result.get_one("prose-wrap").map(|s| s.to_string());

  let no_semicolons = if result.contains("no-semicolons") {
    Some(!matches!(result.get_one("no-semicolons"), Some("false")))
  } else {
    None
  };

  let unstable_component = result.get_bool("unstable-component");
  let unstable_sql = result.get_bool("unstable-sql");

  flags.subcommand = DenoSubcommand::Fmt(FmtFlags {
    check: result.get_bool("check"),
    fail_fast: result.get_bool("fail-fast"),
    files: FileFlags { include, ignore },
    permit_no_files: result.get_bool("permit-no-files"),
    use_tabs,
    line_width,
    indent_width,
    single_quote,
    prose_wrap,
    no_semicolons,
    watch: watch_arg_parse(result),
    unstable_component,
    unstable_sql,
  });
}

fn lint_parse(result: &ParseResult, flags: &mut Flags) {
  config_args_parse(result, flags);
  ext_arg_parse(result, flags);
  allow_and_deny_import_parse(result, flags);

  let files = result
    .get_many("files")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();
  let ignore = result
    .get_many("ignore")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();

  let fix = result.get_bool("fix");
  let rules = result.get_bool("rules");
  let maybe_rules_tags = result
    .get_many("rules-tags")
    .map(|v| v.iter().map(|s| s.to_string()).collect());
  let maybe_rules_include = result
    .get_many("rules-include")
    .map(|v| v.iter().map(|s| s.to_string()).collect());
  let maybe_rules_exclude = result
    .get_many("rules-exclude")
    .map(|v| v.iter().map(|s| s.to_string()).collect());
  let json = result.get_bool("json");
  let compact = result.get_bool("compact");

  flags.subcommand = DenoSubcommand::Lint(LintFlags {
    files: FileFlags {
      include: files,
      ignore,
    },
    fix,
    rules,
    maybe_rules_tags,
    maybe_rules_include,
    maybe_rules_exclude,
    permit_no_files: result.get_bool("permit-no-files"),
    json,
    compact,
    watch: watch_arg_parse(result),
  });
}

fn test_parse(result: &ParseResult, flags: &mut Flags) {
  flags.type_check_mode = TypeCheckMode::Local;
  runtime_args_parse(result, flags, true, true, true);
  ext_arg_parse(result, flags);

  // deno test always uses --no-prompt
  flags.permissions.no_prompt = true;

  let ignore = result
    .get_many("ignore")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();

  let no_run = result.get_bool("no-run");
  let trace_leaks = result.get_bool("trace-leaks");
  let doc = result.get_bool("doc");
  let filter = result.get_one("filter").map(|s| s.to_string());
  let clean = result.get_bool("clean");

  let fail_fast = if result.contains("fail-fast") {
    let val = result
      .get_one("fail-fast")
      .and_then(|s| s.parse::<usize>().ok())
      .unwrap_or(1);
    NonZeroUsize::new(val)
  } else {
    None
  };

  let shuffle = if result.contains("shuffle") {
    let val = result
      .get_one("shuffle")
      .and_then(|s| s.parse::<u64>().ok())
      .unwrap_or(0);
    Some(val)
  } else {
    None
  };

  let include = result
    .get_many("files")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();

  let junit_path = result.get_one("junit-path").map(|s| s.to_string());

  let reporter = if let Some(reporter_str) = result.get_one("reporter") {
    match reporter_str {
      "pretty" => TestReporterConfig::Pretty,
      "junit" => TestReporterConfig::Junit,
      "dot" => TestReporterConfig::Dot,
      "tap" => TestReporterConfig::Tap,
      _ => TestReporterConfig::Pretty,
    }
  } else {
    TestReporterConfig::Pretty
  };

  if matches!(reporter, TestReporterConfig::Dot | TestReporterConfig::Tap) {
    flags.log_level = Some(log::Level::Error);
  }

  let hide_stacktraces = result.get_bool("hide-stacktraces");

  let coverage_dir = if result.contains("coverage") {
    Some(result.get_one("coverage").unwrap_or("coverage").to_string())
  } else {
    None
  };

  flags.subcommand = DenoSubcommand::Test(TestFlags {
    no_run,
    doc,
    coverage_dir,
    coverage_raw_data_only: result.get_bool("coverage-raw-data-only"),
    clean,
    fail_fast,
    files: FileFlags { include, ignore },
    filter,
    shuffle,
    permit_no_files: result.get_bool("permit-no-files"),
    parallel: result.get_bool("parallel"),
    trace_leaks,
    watch: watch_arg_parse_with_paths(result),
    reporter,
    junit_path,
    hide_stacktraces,
  });
}

fn upgrade_parse(
  result: &ParseResult,
  flags: &mut Flags,
) -> Result<(), CliError> {
  ca_file_arg_parse(result, flags);
  unsafely_ignore_certificate_errors_parse(result, flags);

  let dry_run = result.get_bool("dry-run");
  let force = result.get_bool("force");
  let canary = result.get_bool("canary");
  let release_candidate = result.get_bool("release-candidate");
  let version = result.get_one("version").map(|s| s.to_string());
  let output = result.get_one("output").map(|s| s.to_string());
  let checksum = result.get_one("checksum").map(|s| s.to_string());

  let positional = result
    .get_one("version-or-hash-or-channel")
    .map(|s| s.to_string());

  // Handle "pr <number>" and "compass" special positional patterns
  let pr_number_positional = result
    .get_one("pr-number-positional")
    .map(|s| s.to_string());

  let (version_or_hash_or_channel, pr, branch) =
    if positional.as_deref() == Some("pr") {
      // "deno upgrade pr <number>" — second positional is the PR number
      let pr_str = pr_number_positional
        .as_deref()
        .or_else(|| result.get_one("pr"));
      match pr_str {
        None => {
          return Err(CliError::new(
            CliErrorKind::MissingRequired,
            "missing PR number for 'deno upgrade pr'",
          ));
        }
        Some(s) => {
          let s = s.strip_prefix('#').unwrap_or(s);
          match s.parse::<u64>() {
            Ok(n) => (None, Some(n), None),
            Err(_) => {
              return Err(CliError::new(
                CliErrorKind::InvalidValue,
                format!("invalid PR number: '{s}'"),
              ));
            }
          }
        }
      }
    } else if positional.as_deref() == Some("compass") {
      (None, None, Some("compass".to_string()))
    } else {
      let pr_flag = result.get_one("pr").and_then(|s| s.parse::<u64>().ok());
      let branch_flag = result.get_one("branch").map(|s| s.to_string());
      if pr_flag.is_some() {
        (None, pr_flag, None)
      } else if branch_flag.is_some() {
        (None, None, branch_flag)
      } else {
        (positional, None, None)
      }
    };

  flags.subcommand = DenoSubcommand::Upgrade(UpgradeFlags {
    dry_run,
    force,
    release_candidate,
    canary,
    version,
    output,
    version_or_hash_or_channel,
    checksum,
    pr,
    branch,
  });
  Ok(())
}

fn cache_parse(result: &ParseResult, flags: &mut Flags) {
  compile_args_parse(result, flags);
  allow_scripts_arg_parse(result, flags);
  allow_and_deny_import_parse(result, flags);
  env_file_arg_parse(result, flags);
  ext_arg_parse(result, flags);

  let files = result
    .get_many("files")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();
  flags.subcommand = DenoSubcommand::Cache(CacheFlags { files });
}

fn check_parse(result: &ParseResult, flags: &mut Flags) {
  flags.type_check_mode = TypeCheckMode::Local;
  compile_args_without_check_parse(result, flags);
  v8_flags_arg_parse(result, flags);

  let files = result
    .get_many("files")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_else(|| vec![".".to_string()]);

  if result.get_bool("all") || result.get_bool("remote") {
    flags.type_check_mode = TypeCheckMode::All;
  }

  flags.subcommand = DenoSubcommand::Check(CheckFlags {
    files,
    doc: result.get_bool("doc"),
    doc_only: result.get_bool("doc-only"),
    check_js: result.get_bool("check-js"),
  });
  flags.code_cache_enabled = !result.get_bool("no-code-cache");
  allow_and_deny_import_parse(result, flags);
}

fn info_parse(result: &ParseResult, flags: &mut Flags) {
  reload_arg_parse(result, flags);
  config_args_parse(result, flags);
  import_map_arg_parse(result, flags);
  location_arg_parse(result, flags);
  ca_file_arg_parse(result, flags);
  unsafely_ignore_certificate_errors_parse(result, flags);
  node_modules_and_vendor_dir_arg_parse(result, flags);
  lock_args_parse(result, flags);
  no_remote_arg_parse(result, flags);
  no_npm_arg_parse(result, flags);
  allow_and_deny_import_parse(result, flags);

  let json = result.get_bool("json");
  flags.subcommand = DenoSubcommand::Info(InfoFlags {
    file: result.get_one("file").map(|s| s.to_string()),
    json,
  });
}

fn doc_parse(result: &ParseResult, flags: &mut Flags) {
  import_map_arg_parse(result, flags);
  reload_arg_parse(result, flags);
  lock_args_parse(result, flags);
  no_npm_arg_parse(result, flags);
  no_remote_arg_parse(result, flags);
  allow_and_deny_import_parse(result, flags);

  let _has_builtin = result.get_bool("builtin");
  let source_files = if let Some(values) = result.get_many("source_file") {
    let paths: Vec<String> = values
      .iter()
      .filter(|v| v.as_str() != "--builtin")
      .map(|s| s.to_string())
      .collect();
    if paths.is_empty() {
      DocSourceFileFlag::Builtin
    } else {
      DocSourceFileFlag::Paths(paths)
    }
  } else {
    DocSourceFileFlag::Builtin
  };

  let private = result.get_bool("private");
  let lint = result.get_bool("lint");
  let json = result.get_bool("json");
  let filter = result.get_one("filter").map(|s| s.to_string());

  let html = if result.get_bool("html") {
    let name = result.get_one("name").map(|s| s.to_string());
    let category_docs_path =
      result.get_one("category-docs").map(|s| s.to_string());
    let symbol_redirect_map_path =
      result.get_one("symbol-redirect-map").map(|s| s.to_string());
    let default_symbol_map_path =
      result.get_one("default-symbol-map").map(|s| s.to_string());
    let strip_trailing_html = result.get_bool("strip-trailing-html");
    let output = result
      .get_one("output")
      .map(|s| s.to_string())
      .unwrap_or_else(|| "./docs/".to_string());
    Some(DocHtmlFlag {
      name,
      category_docs_path,
      symbol_redirect_map_path,
      default_symbol_map_path,
      strip_trailing_html,
      output,
    })
  } else {
    None
  };

  flags.subcommand = DenoSubcommand::Doc(DocFlags {
    source_files,
    json,
    lint,
    html,
    filter,
    private,
  });
}

fn task_parse(result: &ParseResult, flags: &mut Flags) {
  config_args_parse(result, flags);
  node_modules_arg_parse(result, flags);
  lock_args_parse(result, flags);

  let mut recursive = result.get_bool("recursive");
  let filter =
    if let Some(filter) = result.get_one("filter").map(|s| s.to_string()) {
      recursive = false;
      Some(filter)
    } else if recursive {
      Some("*".to_string())
    } else {
      None
    };

  flags.tunnel = result.get_bool("tunnel");

  let task_name = result.get_one("task_name").map(|s| s.to_string());
  let eval = result.get_bool("eval");

  flags.subcommand = DenoSubcommand::Task(TaskFlags {
    cwd: result.get_one("cwd").map(|s| s.to_string()),
    task: task_name,
    is_run: false,
    recursive,
    filter,
    eval,
  });
}

fn bench_parse(result: &ParseResult, flags: &mut Flags) {
  flags.type_check_mode = TypeCheckMode::Local;
  runtime_args_parse(result, flags, true, false, true);
  ext_arg_parse(result, flags);

  // bench always uses --no-prompt
  flags.permissions.no_prompt = true;

  let json = result.get_bool("json");
  let ignore = result
    .get_many("ignore")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();
  let filter = result.get_one("filter").map(|s| s.to_string());
  let include = result
    .get_many("files")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();
  let no_run = result.get_bool("no-run");

  flags.subcommand = DenoSubcommand::Bench(BenchFlags {
    files: FileFlags { include, ignore },
    filter,
    json,
    no_run,
    permit_no_files: result.get_bool("permit-no-files"),
    watch: watch_arg_parse(result),
  });
}

fn compile_parse(result: &ParseResult, flags: &mut Flags) {
  flags.type_check_mode = TypeCheckMode::Local;
  runtime_args_parse(result, flags, true, false, true);

  let source_file = result
    .get_one("source_file")
    .map(|s| s.to_string())
    .unwrap_or_default();
  let output = result.get_one("output").map(|s| s.to_string());
  let target = result.get_one("target").map(|s| s.to_string());
  let icon = result.get_one("icon").map(|s| s.to_string());
  let no_terminal = result.get_bool("no-terminal");
  let eszip = result.get_bool("eszip-internal-do-not-use");
  let self_extracting = result.get_bool("self-extracting");

  let include = result
    .get_many("include")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();
  let exclude = result
    .get_many("exclude")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();

  ext_arg_parse(result, flags);
  flags.code_cache_enabled = !result.get_bool("no-code-cache");

  // Trailing args are the compile args
  let args = result.trailing.clone();

  flags.subcommand = DenoSubcommand::Compile(CompileFlags {
    source_file,
    output,
    args,
    target,
    no_terminal,
    icon,
    include,
    exclude,
    eszip,
    self_extracting,
  });
}

fn coverage_parse(result: &ParseResult, flags: &mut Flags) {
  let files = result
    .get_many("files")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_else(|| vec!["coverage".to_string()]);
  let ignore = result
    .get_many("ignore")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();
  let include = result
    .get_many("include")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_else(|| vec!["^file:".to_string()]);
  let exclude = result
    .get_many("exclude")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_else(|| vec![r"test\.(js|mjs|ts|jsx|tsx)$".to_string()]);

  let r#type = if result.get_bool("lcov") {
    CoverageType::Lcov
  } else if result.get_bool("html") {
    CoverageType::Html
  } else if result.get_bool("detailed") {
    CoverageType::Detailed
  } else {
    CoverageType::Summary
  };

  let output = result.get_one("output").map(|s| s.to_string());

  flags.subcommand = DenoSubcommand::Coverage(CoverageFlags {
    files: FileFlags {
      include: files,
      ignore,
    },
    output,
    include,
    exclude,
    r#type,
  });
}

fn repl_parse(result: &ParseResult, flags: &mut Flags, is_default: bool) {
  compile_args_without_check_parse(result, flags);
  cached_only_arg_parse(result, flags);
  permission_args_parse(result, flags);
  inspect_arg_parse(result, flags);
  location_arg_parse(result, flags);
  v8_flags_arg_parse(result, flags);
  seed_arg_parse(result, flags);
  enable_testing_features_arg_parse(result, flags);
  env_file_arg_parse(result, flags);
  trace_ops_parse(result, flags);

  let eval_files = result
    .get_many("eval-file")
    .map(|v| v.iter().map(|s| s.to_string()).collect());

  let eval = result.get_one("eval").map(|s| s.to_string());
  let json = result.get_bool("json");

  let repl_flags = ReplFlags {
    eval_files,
    eval,
    is_default_command: is_default,
    json,
  };

  // If user runs bare `deno`, allow all permissions.
  if repl_flags.is_default_command {
    flags.permissions.allow_all = true;
  }

  flags.subcommand = DenoSubcommand::Repl(repl_flags);
}

fn install_parse(
  result: &ParseResult,
  flags: &mut Flags,
) -> Result<(), CliError> {
  runtime_args_parse(result, flags, true, true, false);

  let global = result.get_bool("global");
  allow_scripts_arg_parse(result, flags);

  if global {
    let root = result.get_one("root").map(|s| s.to_string());
    let force = result.get_bool("force");
    let compile = result.get_bool("compile");
    let name = result.get_one("name").map(|s| s.to_string());
    let module_urls = result
      .get_many("cmd")
      .map(|v| v.iter().map(|s| s.to_string()).collect())
      .unwrap_or_default();
    let args = result.trailing.clone();

    flags.subcommand =
      DenoSubcommand::Install(InstallFlags::Global(InstallFlagsGlobal {
        name,
        module_urls,
        args,
        root,
        force,
        compile,
      }));
    return Ok(());
  }

  // Permission flags (other than --allow-import) are only valid for global installs.
  {
    let perm_flags = [
      "allow-all", "allow-read", "deny-read", "allow-write", "deny-write",
      "allow-net", "deny-net", "allow-env", "deny-env", "allow-run",
      "deny-run", "allow-sys", "deny-sys", "allow-ffi", "deny-ffi",
    ];
    for flag in &perm_flags {
      if result.contains(flag) {
        return Err(CliError::new(
          CliErrorKind::InvalidValue,
          format!(
            "Note: Permission flags can only be used in a global setting\n\
             Use `deno install -g --allow-net jsr:@std/fs` instead"
          ),
        ));
      }
    }
  }

  let lockfile_only = result.get_bool("lockfile-only");

  if result.contains("entrypoint") {
    // --entrypoint takes values directly; also include any positional "cmd" args
    let mut entrypoints: Vec<String> = result
      .get_many("entrypoint")
      .map(|v| v.iter().map(|s| s.to_string()).collect())
      .unwrap_or_default();
    if let Some(cmd_vals) = result.get_many("cmd") {
      entrypoints.extend(cmd_vals.iter().map(|s| s.to_string()));
    }
    flags.subcommand = DenoSubcommand::Install(InstallFlags::Local(
      InstallFlagsLocal::Entrypoints(InstallEntrypointsFlags {
        entrypoints,
        lockfile_only,
      }),
    ));
  } else if let Some(packages) = result.get_many("cmd") {
    if !packages.is_empty() {
      let dev = result.get_bool("dev");
      let default_registry = if result.get_bool("npm") {
        Some(DefaultRegistry::Npm)
      } else if result.get_bool("jsr") {
        Some(DefaultRegistry::Jsr)
      } else {
        None
      };
      flags.subcommand = DenoSubcommand::Install(InstallFlags::Local(
        InstallFlagsLocal::Add(AddFlags {
          packages: packages.iter().map(|s| s.to_string()).collect(),
          dev,
          default_registry,
          lockfile_only,
          save_exact: result.get_bool("save-exact"),
        }),
      ));
    } else {
      flags.subcommand = DenoSubcommand::Install(InstallFlags::Local(
        InstallFlagsLocal::TopLevel(InstallTopLevelFlags { lockfile_only }),
      ));
    }
  } else {
    flags.subcommand = DenoSubcommand::Install(InstallFlags::Local(
      InstallFlagsLocal::TopLevel(InstallTopLevelFlags { lockfile_only }),
    ));
  }
  Ok(())
}

fn uninstall_parse(result: &ParseResult, flags: &mut Flags) {
  lock_args_parse(result, flags);

  let global = result.get_bool("global");
  let packages = result
    .get_many("packages")
    .map(|v| v.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    .unwrap_or_default();

  let kind = if global {
    let root = result.get_one("root").map(|s| s.to_string());
    let name = packages.into_iter().next().unwrap_or_default();
    UninstallKind::Global(UninstallFlagsGlobal { name, root })
  } else {
    UninstallKind::Local(RemoveFlags {
      packages,
      lockfile_only: result.get_bool("lockfile-only"),
    })
  };

  flags.subcommand = DenoSubcommand::Uninstall(UninstallFlags { kind });
}

fn completions_parse(result: &ParseResult, flags: &mut Flags) {
  let shell = result
    .get_one("shell")
    .map(|s| s.to_string())
    .unwrap_or_else(|| "bash".to_string());
  let buf = crate::completions::generate(&shell, &crate::defs::DENO_ROOT);
  flags.subcommand = DenoSubcommand::Completions(CompletionsFlags::Static(
    buf.into_boxed_slice(),
  ));
}

fn init_parse(result: &ParseResult, flags: &mut Flags) {
  let mut lib = result.get_bool("lib");
  let mut serve = result.get_bool("serve");
  let mut empty = result.get_bool("empty");
  let mut yes = result.get_bool("yes");

  let use_npm = result.get_bool("npm");
  let use_jsr = result.get_bool("jsr");

  let mut dir = None;
  let mut package = None;
  let mut package_args = vec![];

  if let Some(args) = result.get_many("args")
    && !args.is_empty()
  {
    let name = args[0].clone();
    let rest: Vec<String> = args[1..].iter().map(|s| s.to_string()).collect();

    if use_npm {
      package = Some(format!(
        "npm:{}",
        name.strip_prefix("npm:").unwrap_or(&name)
      ));
      package_args = rest;
    } else if use_jsr {
      package = Some(format!(
        "jsr:{}",
        name.strip_prefix("jsr:").unwrap_or(&name)
      ));
      package_args = rest;
    } else {
      dir = Some(name);
      // Per-positional trailing captures flags like --lib after
      // the dir positional. Re-parse them as init flags
      // (matching the original clap sub-parse behavior).
      for extra in &rest {
        match extra.as_str() {
          "--lib" => lib = true,
          "--serve" => serve = true,
          "--empty" => empty = true,
          "--yes" | "-y" => yes = true,
          _ => {}
        }
      }
    }
  }

  flags.subcommand = DenoSubcommand::Init(InitFlags {
    package,
    package_args,
    dir,
    lib,
    serve,
    empty,
    yes,
  });
}

fn create_parse(result: &ParseResult, flags: &mut Flags) {
  let package = result
    .get_one("package")
    .map(|s| s.to_string())
    .unwrap_or_default();
  let use_npm = result.get_bool("npm");
  let use_jsr = result.get_bool("jsr");
  // With per-positional trailing on package_args, values after the
  // package positional (including after --) go directly into
  // the package_args positional's values.
  let package_args: Vec<String> = result
    .get_many("package_args")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();

  let package = if package.starts_with("jsr:") || package.starts_with("npm:") {
    package
  } else if use_npm {
    format!("npm:{package}")
  } else if use_jsr {
    format!("jsr:{package}")
  } else {
    package
  };

  flags.subcommand = DenoSubcommand::Init(InitFlags {
    package: Some(package),
    package_args,
    dir: None,
    lib: false,
    serve: false,
    empty: false,
    yes: result.get_bool("yes"),
  });
}

fn jupyter_parse(result: &ParseResult, flags: &mut Flags) {
  let conn_file = result.get_one("conn").map(|s| s.to_string());
  let kernel = result.get_bool("kernel");
  let install = result.get_bool("install");
  let display = result.get_one("display").map(|s| s.to_string());
  let name = result.get_one("name").map(|s| s.to_string());
  let force = result.get_bool("force");

  flags.subcommand = DenoSubcommand::Jupyter(JupyterFlags {
    install,
    kernel,
    conn_file,
    name,
    display,
    force,
  });
}

fn publish_parse(result: &ParseResult, flags: &mut Flags) {
  flags.type_check_mode = TypeCheckMode::Local;
  no_check_arg_parse(result, flags);
  check_arg_parse(result, flags);
  config_args_parse(result, flags);

  flags.subcommand = DenoSubcommand::Publish(PublishFlags {
    token: result.get_one("token").map(|s| s.to_string()),
    dry_run: result.get_bool("dry-run"),
    allow_slow_types: result.get_bool("allow-slow-types"),
    allow_dirty: result.get_bool("allow-dirty"),
    no_provenance: result.get_bool("no-provenance"),
    set_version: result.get_one("set-version").map(|s| s.to_string()),
  });
}

fn add_parse(result: &ParseResult, flags: &mut Flags) {
  allow_scripts_arg_parse(result, flags);
  lock_args_parse(result, flags);

  let packages = result
    .get_many("packages")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();
  let dev = result.get_bool("dev");
  let default_registry = if result.get_bool("npm") {
    Some(DefaultRegistry::Npm)
  } else if result.get_bool("jsr") {
    Some(DefaultRegistry::Jsr)
  } else {
    None
  };

  flags.subcommand = DenoSubcommand::Add(AddFlags {
    packages,
    dev,
    default_registry,
    lockfile_only: result.get_bool("lockfile-only"),
    save_exact: result.get_bool("save-exact"),
  });
}

fn remove_parse(result: &ParseResult, flags: &mut Flags) {
  lock_args_parse(result, flags);
  flags.subcommand = DenoSubcommand::Remove(RemoveFlags {
    packages: result
      .get_many("packages")
      .map(|v| v.iter().map(|s| s.to_string()).collect())
      .unwrap_or_default(),
    lockfile_only: result.get_bool("lockfile-only"),
  });
}

fn outdated_parse(result: &ParseResult, flags: &mut Flags, is_update: bool) {
  let filters = result
    .get_many("filters")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();
  let recursive = result.get_bool("recursive");

  let update_flag = result.get_bool("update");
  let kind = if is_update || update_flag {
    let latest = result.get_bool("latest");
    let interactive = result.get_bool("interactive");
    OutdatedKind::Update {
      latest,
      interactive,
      lockfile_only: result.get_bool("lockfile-only"),
    }
  } else {
    let compatible = result.get_bool("compatible");
    OutdatedKind::PrintOutdated { compatible }
  };

  flags.subcommand = DenoSubcommand::Outdated(OutdatedFlags {
    filters,
    recursive,
    kind,
  });

  lock_args_parse(result, flags);
  min_dep_age_arg_parse(result, flags);
}

fn clean_parse(result: &ParseResult, flags: &mut Flags) {
  node_modules_and_vendor_dir_arg_parse(result, flags);

  let except_paths = result
    .get_many("except-paths")
    .map(|v| v.iter().map(|s| s.to_string()).collect())
    .unwrap_or_default();
  let dry_run = result.get_bool("dry-run");

  flags.subcommand = DenoSubcommand::Clean(CleanFlags {
    except_paths,
    dry_run,
  });
}

fn approve_scripts_parse(result: &ParseResult, flags: &mut Flags) {
  flags.subcommand = DenoSubcommand::ApproveScripts(ApproveScriptsFlags {
    packages: result
      .get_many("packages")
      .map(|v| v.iter().map(|s| s.to_string()).collect())
      .unwrap_or_default(),
    lockfile_only: result.get_bool("lockfile-only"),
  });
}

fn types_parse(flags: &mut Flags) {
  flags.subcommand = DenoSubcommand::Types;
}

fn lsp_parse(flags: &mut Flags) {
  flags.subcommand = DenoSubcommand::Lsp;
}

fn vendor_parse(flags: &mut Flags) {
  flags.subcommand = DenoSubcommand::Vendor;
}

fn deploy_parse(result: &ParseResult, flags: &mut Flags, sandbox: bool) {
  // deploy/sandbox are passthrough — all args go into argv
  flags.argv = result.trailing.clone();
  flags.subcommand = DenoSubcommand::Deploy(DeployFlags { sandbox });
}

// ============================================================
// NODE_OPTIONS support
// ============================================================

#[cfg(test)]
thread_local! {
    static TEST_NODE_OPTIONS: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_test_node_options(value: Option<&str>) {
  TEST_NODE_OPTIONS.with(|opt| {
    *opt.borrow_mut() = value.map(|s| s.to_string());
  });
}

/// Parse NODE_OPTIONS environment variable into a list of arguments.
/// Handles quoted strings and backslash escapes, matching Node.js behavior.
pub fn parse_node_options_env_var(
  node_options: &str,
) -> Result<Vec<String>, String> {
  let mut env_argv = Vec::new();
  let mut is_in_string = false;
  let mut will_start_new_arg = true;

  let chars: Vec<char> = node_options.chars().collect();
  let mut index = 0;

  while index < chars.len() {
    let mut c = chars[index];

    // Backslashes escape the following character
    if c == '\\' && is_in_string {
      if index + 1 == chars.len() {
        return Err(
          "invalid value for NODE_OPTIONS (invalid escape)".to_string(),
        );
      } else {
        index += 1;
        c = chars[index];
      }
    } else if c == ' ' && !is_in_string {
      will_start_new_arg = true;
      index += 1;
      continue;
    } else if c == '"' {
      is_in_string = !is_in_string;
      index += 1;
      continue;
    }

    if will_start_new_arg {
      env_argv.push(c.to_string());
      will_start_new_arg = false;
    } else if let Some(last) = env_argv.last_mut() {
      last.push(c);
    }

    index += 1;
  }

  if is_in_string {
    return Err(
      "invalid value for NODE_OPTIONS (unterminated string)".to_string(),
    );
  }

  Ok(env_argv)
}

/// Reads some flags from NODE_OPTIONS:
/// https://nodejs.org/api/cli.html#node_optionsoptions
/// Currently supports:
/// - `--require` / `-r`
/// - `--inspect-publish-uid`
#[allow(clippy::disallowed_methods, reason = "reads NODE_OPTIONS from env")]
fn apply_node_options(flags: &mut Flags) {
  let node_options = match std::env::var("NODE_OPTIONS") {
    Ok(val) if !val.is_empty() => val,
    _ => {
      #[cfg(test)]
      {
        match TEST_NODE_OPTIONS.with(|opt| opt.borrow().clone()) {
          Some(val) if !val.is_empty() => val,
          _ => return,
        }
      }
      #[cfg(not(test))]
      return;
    }
  };

  let args = match parse_node_options_env_var(&node_options) {
    Ok(args) => args,
    Err(_) => return,
  };

  // Filter to only supported flags, using a scan to track whether the
  // previous token was --require/-r (so we keep its value argument too).
  let filtered: Vec<&str> = args
    .iter()
    .map(String::as_str)
    .scan(false, |prev_was_require, word| {
      if word == "--require" || word == "-r" {
        *prev_was_require = true;
        return Some((word, true));
      }
      if word.starts_with("--inspect-publish-uid=") || *prev_was_require {
        *prev_was_require = false;
        return Some((word, true));
      }
      *prev_was_require = false;
      Some((word, false))
    })
    .filter_map(|(word, should_keep)| should_keep.then_some(word))
    .collect();

  // Parse --require values
  let mut node_require: Vec<String> = Vec::new();
  let mut i = 0;
  while i < filtered.len() {
    let word = filtered[i];
    if (word == "--require" || word == "-r") && i + 1 < filtered.len() {
      node_require.push(filtered[i + 1].to_string());
      i += 2;
    } else if word.starts_with("--inspect-publish-uid=") {
      // Handle below
      i += 1;
    } else {
      i += 1;
    }
  }

  // Prepend NODE_OPTIONS --require values before CLI --require values
  if !node_require.is_empty() {
    node_require.append(&mut flags.require);
    flags.require = node_require;
  }

  // Parse --inspect-publish-uid (only if not already set from CLI)
  if flags.inspect_publish_uid.is_none() {
    for word in &filtered {
      if let Some(value) = word.strip_prefix("--inspect-publish-uid=") {
        let mut uid = InspectPublishUid {
          console: false,
          http: false,
        };
        for part in value.split(',') {
          match part.trim() {
            "stderr" | "console" => uid.console = true,
            "http" => uid.http = true,
            _ => {}
          }
        }
        flags.inspect_publish_uid = Some(uid);
        break;
      }
    }
  }
}

fn bundle_parse(result: &ParseResult, flags: &mut Flags) {
  compile_args_without_check_parse(result, flags);
  permission_args_parse(result, flags);
  allow_scripts_arg_parse(result, flags);

  let entrypoints = result
    .get_many("file")
    .map(|v| v.to_vec())
    .unwrap_or_default();
  let output_path = result.get_one("output").map(|s| s.to_string());
  let output_dir = result.get_one("outdir").map(|s| s.to_string());
  let format_str = result.get_one("format").unwrap_or("esm");
  let format = match format_str {
    "cjs" => BundleFormat::Cjs,
    "iife" => BundleFormat::Iife,
    _ => BundleFormat::Esm,
  };
  let packages_str = result.get_one("packages").unwrap_or("bundle");
  let packages = match packages_str {
    "external" => PackageHandling::External,
    _ => PackageHandling::Bundle,
  };
  let platform_str = result.get_one("platform").unwrap_or("deno");
  let platform = match platform_str {
    "browser" => BundlePlatform::Browser,
    _ => BundlePlatform::Deno,
  };
  let sourcemap = result.get_one("sourcemap").map(|s| match s {
    "inline" => SourceMapType::Inline,
    "external" => SourceMapType::External,
    _ => SourceMapType::Linked,
  });
  let external = result
    .get_many("external")
    .map(|v| v.to_vec())
    .unwrap_or_default();

  flags.subcommand = DenoSubcommand::Bundle(BundleFlags {
    entrypoints,
    output_path,
    output_dir,
    external,
    format,
    packages,
    platform,
    sourcemap,
    watch: result.get_bool("watch"),
    minify: result.get_bool("minify"),
    keep_names: result.get_bool("keep-names"),
    code_splitting: result.get_bool("code-splitting"),
    inline_imports: !matches!(result.get_one("inline-imports"), Some("false")),
  });
}

fn audit_parse(result: &ParseResult, flags: &mut Flags) {
  lock_args_parse(result, flags);

  let severity = result.get_one("level").unwrap_or("low").to_string();
  let ignore_unfixable = result.get_bool("ignore-unfixable");
  let ignore_registry_errors = result.get_bool("ignore-registry-errors");
  let socket = result.get_bool("socket");
  let ignore = result
    .get_many("ignore")
    .map(|v| v.to_vec())
    .unwrap_or_default();

  flags.subcommand = DenoSubcommand::Audit(AuditFlags {
    severity,
    dev: true,
    prod: true,
    optional: true,
    ignore_registry_errors,
    ignore_unfixable,
    ignore,
    socket,
  });
}

fn x_parse(result: &ParseResult, flags: &mut Flags) {
  let kind = if let Some(alias) = result.get_one("install-alias") {
    if !result.contains("install-alias") {
      // Not provided, check for script_arg
      goto_script(result, flags)
    } else {
      let name = match alias {
        "dx" => DenoXShimName::Dx,
        "denox" => DenoXShimName::Denox,
        "dnx" => DenoXShimName::Dnx,
        other => DenoXShimName::Other(other.to_string()),
      };
      XFlagsKind::InstallAlias(name)
    }
  } else {
    goto_script(result, flags)
  };

  fn goto_script(result: &ParseResult, flags: &mut Flags) -> XFlagsKind {
    if let Some(args) = result.get_many("script_arg") {
      if !args.is_empty() {
        let command = args[0].clone();
        let yes = result.get_bool("yes");
        runtime_args_parse(result, flags, true, true, true);
        permission_args_parse(result, flags);
        flags.argv.extend(args[1..].iter().cloned());
        XFlagsKind::Command(XCommandFlags { yes, command })
      } else {
        XFlagsKind::Print
      }
    } else {
      XFlagsKind::Print
    }
  }

  if !flags.permissions.has_permission() && flags.permission_set.is_none() {
    flags.permissions.allow_all = true;
  }
  flags.subcommand = DenoSubcommand::X(XFlags { kind });
}

fn json_reference_parse(flags: &mut Flags) {
  // Build a JSON representation of all commands from our static definitions.
  // This replaces the clap-based json_reference_parse.
  let root = &crate::defs::DENO_ROOT;
  let json_str = serialize_command_def(root, true);
  let json = deno_core::serde_json::from_str(&json_str)
    .unwrap_or_else(|_| deno_core::serde_json::Value::Null);
  flags.subcommand = DenoSubcommand::JSONReference(JSONReferenceFlags { json });
}

fn serialize_command_def(cmd: &crate::CommandDef, top_level: bool) -> String {
  use std::fmt::Write;
  let mut out = String::new();
  out.push('{');

  // name
  write!(out, "\"name\":{}", json_escape(cmd.name)).unwrap();

  // about
  write!(
    out,
    ",\"about\":{}",
    json_escape_option(if cmd.about.is_empty() {
      None
    } else {
      Some(cmd.about)
    })
  )
  .unwrap();

  // usage
  let usage = format!("Usage: {}", cmd.name);
  write!(out, ",\"usage\":{}", json_escape(&usage)).unwrap();

  // args
  let args: Vec<&crate::ArgDef> = if top_level {
    cmd.all_args().filter(|a| a.global && !a.hidden).collect()
  } else {
    cmd.all_args().filter(|a| !a.global && !a.hidden).collect()
  };

  out.push_str(",\"args\":[");
  for (i, arg) in args.iter().enumerate() {
    if i > 0 {
      out.push(',');
    }
    out.push('{');
    write!(out, "\"name\":{}", json_escape(arg.name)).unwrap();
    if let Some(short) = arg.short {
      write!(out, ",\"short\":\"{}\"", short).unwrap();
    } else {
      out.push_str(",\"short\":null");
    }
    if let Some(long) = arg.long {
      write!(out, ",\"long\":{}", json_escape(long)).unwrap();
    } else {
      out.push_str(",\"long\":null");
    }
    write!(out, ",\"required\":{}", arg.required).unwrap();
    write!(
      out,
      ",\"help\":{}",
      json_escape_option(if arg.help.is_empty() {
        None
      } else {
        Some(arg.help)
      })
    )
    .unwrap();
    out.push_str(",\"help_heading\":null");

    // usage string
    let mut usage = String::new();
    if let Some(short) = arg.short {
      write!(usage, "-{}", short).unwrap();
      if arg.long.is_some() {
        usage.push_str(", ");
      }
    }
    if let Some(long) = arg.long {
      write!(usage, "--{}", long).unwrap();
    }
    write!(out, ",\"usage\":{}", json_escape(&usage)).unwrap();

    out.push('}');
  }
  out.push(']');

  // subcommands
  out.push_str(",\"subcommands\":[");
  let visible_subs: Vec<&crate::CommandDef> = cmd
    .subcommands
    .iter()
    .filter(|s| !s.name.starts_with("json_reference") && s.name != "help")
    .collect();
  for (i, sub) in visible_subs.iter().enumerate() {
    if i > 0 {
      out.push(',');
    }
    out.push_str(&serialize_command_def(sub, false));
  }
  out.push(']');

  out.push('}');
  out
}

fn json_escape(s: &str) -> String {
  let mut out = String::with_capacity(s.len() + 2);
  out.push('"');
  for c in s.chars() {
    match c {
      '"' => out.push_str("\\\""),
      '\\' => out.push_str("\\\\"),
      '\n' => out.push_str("\\n"),
      '\r' => out.push_str("\\r"),
      '\t' => out.push_str("\\t"),
      c if c < '\u{20}' => {
        use std::fmt::Write;
        write!(out, "\\u{:04x}", c as u32).unwrap();
      }
      c => out.push(c),
    }
  }
  out.push('"');
  out
}

fn json_escape_option(s: Option<&str>) -> String {
  match s {
    Some(s) => json_escape(s),
    None => "null".to_string(),
  }
}

fn help_subcommand_parse(result: &ParseResult, flags: &mut Flags) {
  let root = &crate::defs::DENO_ROOT;
  let sub_name = result.get_one("subcommand");
  let cmd = sub_name
    .and_then(|name| root.find_subcommand(name))
    .unwrap_or(root);
  let help_text = crate::help::render_help(cmd);
  flags.subcommand = DenoSubcommand::Help(HelpFlags { help: help_text });
}
