// Copyright 2018-2026 the Deno authors. MIT license.

//! Lets Deno stand in for `node` when no real Node.js binary is available.
//!
//! Two complementary mechanisms:
//!
//! 1. Arg0 dispatch ([`maybe_rewrite_node_arg0`]): when the deno binary is
//!    invoked through a file named `node` (a symlink/hardlink created below),
//!    translate the Node.js CLI arguments to Deno arguments and run as if
//!    `deno node ...` had been invoked.
//!
//! 2. PATH injection ([`ensure_node_on_path`]): create a `node` executable
//!    pointing back at the current deno binary in a cache directory and prepend
//!    that directory to the process's own `PATH`, so that child processes
//!    (including native ones such as Next.js Turbopack, which spawn `node` via
//!    a raw OS PATH lookup that bypasses Deno's JS-level interception) can find
//!    a `node` to run.
//!
//! Both are best-effort and only kick in when a real `node` is not already
//! available on `PATH`, so existing Node.js setups are never shadowed. The
//! behavior can be disabled entirely with `DENO_DISABLE_NODE_SHIM=1`.

use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

/// Env var that disables the whole feature when set to a truthy value.
const DISABLE_ENV_VAR: &str = "DENO_DISABLE_NODE_SHIM";
/// Internal marker set once the shim has been put on PATH, used to avoid the
/// shim recursively validating against itself when a spawned `node` (which is
/// really deno) re-enters this code.
const ACTIVE_ENV_VAR: &str = "DENO_NODE_SHIM_ACTIVE";
/// Name of the directory under DENO_DIR that holds the `node` shim.
const SHIM_DIR_NAME: &str = "node_compat_bin";

fn is_truthy(value: &str) -> bool {
  matches!(value, "1" | "true" | "TRUE" | "True")
}

fn env_disabled() -> bool {
  std::env::var(DISABLE_ENV_VAR)
    .map(|v| is_truthy(&v))
    .unwrap_or(false)
}

/// Returns whether `arg0`'s file name is exactly `node` (or `node.exe` on
/// Windows). Deliberately an exact match, not a suffix match, so names like
/// `anode` or `mynode` do not trigger dispatch.
fn is_node_arg0(arg0: &OsString) -> bool {
  let Some(file_name) = Path::new(arg0).file_name() else {
    return false;
  };
  // Never misfire on the deno binary itself.
  if file_name.eq_ignore_ascii_case("deno")
    || file_name.eq_ignore_ascii_case("deno.exe")
  {
    return false;
  }
  if file_name == "node" {
    return true;
  }
  cfg!(windows) && file_name.eq_ignore_ascii_case("node.exe")
}

/// If the process was invoked as `node`, translate the Node.js CLI args into
/// Deno args and return the rewritten argv. Otherwise returns `args` unchanged.
///
/// Must be called before any threads are spawned, as it may mutate process
/// environment variables (`NODE_OPTIONS`, `DENO_TLS_CA_STORE`, ...).
pub fn maybe_rewrite_node_arg0(args: Vec<OsString>) -> Vec<OsString> {
  let Some(arg0) = args.first() else {
    return args;
  };
  if !is_node_arg0(arg0) {
    return args;
  }

  // node_shim operates on Strings. Node never passes non-UTF8 flags, and any
  // entrypoint path is re-resolved by deno's own `run` resolution.
  let node_args: Vec<String> = args[1..]
    .iter()
    .map(|a| a.to_string_lossy().into_owned())
    .collect();

  let parsed = match node_shim::parse_args(node_args) {
    Ok(parsed) => parsed,
    Err(errors) => {
      // This runs before logging is initialized; mirror the standalone shim.
      #[allow(clippy::print_stderr, reason = "node shim arg parse error")]
      {
        if errors.len() == 1 {
          eprintln!("Error: {}", errors[0]);
        } else if errors.len() > 1 {
          eprintln!("Errors: {}", errors.join(", "));
        }
      }
      deno_runtime::exit(1);
    }
  };

  let options = node_shim::TranslateOptions::for_node_cli();
  let result = node_shim::translate_to_deno_args(parsed, &options);

  apply_env_side_effects(&result);

  let mut deno_args = result.deno_args;

  // Resolve the entrypoint for the run path the same way the standalone shim
  // does, sharing a single implementation.
  if deno_args.len() >= 3 && deno_args.get(1).map(|s| s.as_str()) == Some("run")
  {
    let entrypoint_idx = deno_args
      .iter()
      .enumerate()
      .skip(2)
      .find(|(_, arg)| !arg.starts_with('-'))
      .map(|(i, _)| i);
    if let Some(idx) = entrypoint_idx {
      #[allow(
        clippy::disallowed_methods,
        reason = "resolving the node shim exe"
      )]
      let current_exe =
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("deno"));
      deno_args[idx] =
        node_shim::resolve_entrypoint(&current_exe, &deno_args[idx]);
    }
  }

  // `deno_args[0]` is the program-name slot ("node") that deno's flag parser
  // skips; `deno_args[1..]` carry the real subcommand. Returning `deno_args`
  // verbatim is exactly what the standalone shim feeds to deno.
  deno_args.into_iter().map(OsString::from).collect()
}

fn apply_env_side_effects(result: &node_shim::TranslatedArgs) {
  if result.use_system_ca {
    // SAFETY: called before any threads are spawned.
    unsafe { std::env::set_var("DENO_TLS_CA_STORE", "system") };
  }
  if !result.node_options.is_empty() {
    let options = result.node_options.join(" ");
    let merged = match std::env::var("NODE_OPTIONS") {
      Ok(existing) if !existing.is_empty() => {
        format!("{existing} {options}")
      }
      _ => options,
    };
    // SAFETY: called before any threads are spawned.
    unsafe { std::env::set_var("NODE_OPTIONS", merged) };
  }
  if !result.trace_event_categories.is_empty() {
    // SAFETY: called before any threads are spawned.
    unsafe {
      std::env::set_var(
        "DENO_NODE_TRACE_EVENT_CATEGORIES",
        &result.trace_event_categories,
      )
    };
  }
}

/// Ensure a `node` executable that points back at the current deno binary is
/// available on `PATH`, so child processes that spawn `node` natively can find
/// one. Best-effort and no-op when a real `node` already exists, when disabled,
/// or when the shim is already active.
///
/// Must be called before any threads are spawned (it mutates `PATH`).
pub fn ensure_node_on_path(deno_dir_root: &Path) -> std::io::Result<()> {
  if env_disabled() {
    return Ok(());
  }
  // Re-entry guard: if a previously-prepended shim `node` re-invoked us, the
  // shim itself is now on PATH; don't recurse or re-validate against ourselves.
  if std::env::var(ACTIVE_ENV_VAR)
    .map(|v| is_truthy(&v))
    .unwrap_or(false)
  {
    return Ok(());
  }
  // Don't shadow a real Node.js install.
  if which::which("node").is_ok() {
    return Ok(());
  }

  #[allow(clippy::disallowed_methods, reason = "resolving the node shim exe")]
  let current_exe = std::env::current_exe()?;
  let current_exe =
    crate::util::fs::canonicalize_path(&current_exe).unwrap_or(current_exe);

  let shim_dir = deno_dir_root.join(SHIM_DIR_NAME);
  let shim_name = if cfg!(windows) { "node.exe" } else { "node" };
  let shim_path = shim_dir.join(shim_name);

  if !shim_is_valid(&shim_path, &current_exe) {
    std::fs::create_dir_all(&shim_dir)?;
    create_shim(&shim_path, &current_exe)?;
  }

  prepend_self_path(&shim_dir);

  // SAFETY: called before any threads are spawned.
  unsafe { std::env::set_var(ACTIVE_ENV_VAR, "1") };

  Ok(())
}

/// Whether an existing shim at `shim_path` already points at `current_exe`.
fn shim_is_valid(shim_path: &Path, current_exe: &Path) -> bool {
  #[cfg(unix)]
  {
    match std::fs::read_link(shim_path) {
      Ok(target) => {
        target == current_exe
          || crate::util::fs::canonicalize_path(&target).ok().as_deref()
            == Some(current_exe)
      }
      Err(_) => false,
    }
  }
  #[cfg(windows)]
  {
    same_file::is_same_file(shim_path, current_exe).unwrap_or(false)
  }
  #[cfg(not(any(unix, windows)))]
  {
    let _ = (shim_path, current_exe);
    false
  }
}

#[cfg(unix)]
fn create_shim(shim_path: &Path, current_exe: &Path) -> std::io::Result<()> {
  match std::os::unix::fs::symlink(current_exe, shim_path) {
    Ok(()) => Ok(()),
    Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
      std::fs::remove_file(shim_path)?;
      std::os::unix::fs::symlink(current_exe, shim_path)
    }
    Err(err) => Err(err),
  }
}

#[cfg(windows)]
fn create_shim(shim_path: &Path, current_exe: &Path) -> std::io::Result<()> {
  // Native CreateProcess / Rust's Command PATH lookup only execute `.exe`, not
  // `.cmd`, so the shim must be a real executable. A hardlink shares the deno
  // binary's bytes with no extra disk; fall back to a copy across volumes.
  if shim_path.exists() {
    let _ = std::fs::remove_file(shim_path);
  }
  match std::fs::hard_link(current_exe, shim_path) {
    Ok(()) => Ok(()),
    Err(_) => {
      // Copy via a unique temp file then atomically rename into place.
      let tmp_path =
        shim_path.with_extension(format!("exe.tmp-{}", std::process::id()));
      std::fs::copy(current_exe, &tmp_path)?;
      std::fs::rename(&tmp_path, shim_path)
    }
  }
}

#[cfg(not(any(unix, windows)))]
fn create_shim(_shim_path: &Path, _current_exe: &Path) -> std::io::Result<()> {
  Err(std::io::Error::new(
    std::io::ErrorKind::Unsupported,
    "node shim is not supported on this platform",
  ))
}

/// Prepend `dir` to the process's own `PATH` (idempotently), so spawned
/// children inherit it.
fn prepend_self_path(dir: &Path) {
  let sep = if cfg!(windows) { ';' } else { ':' };
  let current = std::env::var_os("PATH").unwrap_or_default();
  // Idempotency: don't grow PATH if the dir is already present.
  let already_present = std::env::split_paths(&current).any(|p| p == dir);
  if already_present {
    return;
  }

  let mut new_path = OsString::from(dir);
  if !current.is_empty() {
    new_path.push(sep.to_string());
    new_path.push(&current);
  }
  // SAFETY: called before any threads are spawned.
  unsafe { std::env::set_var("PATH", new_path) };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn detects_node_arg0() {
    assert!(is_node_arg0(&OsString::from("node")));
    assert!(is_node_arg0(&OsString::from("/usr/local/bin/node")));
    assert!(!is_node_arg0(&OsString::from("anode")));
    assert!(!is_node_arg0(&OsString::from("/path/to/mynode")));
    assert!(!is_node_arg0(&OsString::from("deno")));
    assert!(!is_node_arg0(&OsString::from("/usr/bin/deno")));
  }

  #[test]
  #[cfg(windows)]
  fn detects_node_exe_arg0_on_windows() {
    assert!(is_node_arg0(&OsString::from("node.exe")));
    assert!(is_node_arg0(&OsString::from("NODE.EXE")));
    assert!(!is_node_arg0(&OsString::from("deno.exe")));
  }

  #[test]
  fn non_node_arg0_passes_through_unchanged() {
    let args = vec![
      OsString::from("/usr/bin/deno"),
      OsString::from("run"),
      OsString::from("main.ts"),
    ];
    let result = maybe_rewrite_node_arg0(args.clone());
    assert_eq!(result, args);
  }

  #[test]
  fn truthy_values() {
    assert!(is_truthy("1"));
    assert!(is_truthy("true"));
    assert!(!is_truthy("0"));
    assert!(!is_truthy(""));
  }
}
