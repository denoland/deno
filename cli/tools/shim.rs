// Copyright 2018-2026 the Deno authors. MIT license.

//! `deno shim` — a migration aid for teams coming from Node.
//!
//! It installs small shim scripts named `npm`, `npx`, `pnpm` and `yarn` into a
//! directory that the user can put on their `PATH`. Each shim forwards back to
//! `deno shim --run <pm> -- <args>` so Deno can translate the invocation to its
//! own equivalent.
//!
//! Fidelity rule: only a curated, high-confidence subset of commands and flags
//! is translated. Anything unrecognized is passed through to the real package
//! manager binary if one exists on `PATH`; otherwise we error clearly. We never
//! silently do the wrong thing.

use std::env;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;

use crate::args::Flags;
use crate::args::ShimFlags;
use crate::args::ShimMode;
use crate::colors;

/// The package managers we provide shims for.
const SHIMMED_PMS: &[&str] = &["npm", "npx", "pnpm", "yarn"];

/// The directory shims are installed into, derived the same way as the global
/// installer root so that `DENO_INSTALL_ROOT` redirects it (handy for tests).
fn shim_dir() -> Result<PathBuf, AnyError> {
  if let Some(env_dir) = env::var_os("DENO_INSTALL_ROOT")
    && !env_dir.is_empty()
  {
    return Ok(PathBuf::from(env_dir).join("shims"));
  }
  // Note: on Windows the $HOME environment variable is non-standard, so use
  // %USERPROFILE% there to match the rest of Deno.
  let home_env_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
  let home = env::var_os(home_env_var)
    .map(PathBuf::from)
    .ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::NotFound,
        format!("${home_env_var} is not defined"),
      )
    })?;
  Ok(home.join(".deno").join("shims"))
}

pub fn shim(
  _flags: Arc<Flags>,
  shim_flags: ShimFlags,
) -> Result<i32, AnyError> {
  match shim_flags.mode {
    ShimMode::Install => install(),
    ShimMode::Uninstall => uninstall(),
    ShimMode::List => list(),
    ShimMode::Run { pm, args } => run(&pm, &args),
  }
}

fn install() -> Result<i32, AnyError> {
  let dir = shim_dir()?;
  std::fs::create_dir_all(&dir)
    .with_context(|| format!("Creating shim directory '{}'", dir.display()))?;
  let exe = env::current_exe().context("Resolving the deno executable path")?;
  for pm in SHIMMED_PMS {
    write_shim(&dir, pm, &exe)?;
  }

  log::info!(
    "{} Installed {} shims to {}",
    colors::green("✓"),
    SHIMMED_PMS.join(", "),
    colors::cyan(dir.to_string_lossy()),
  );
  log::info!("");
  log::info!(
    "These are {}, not the real package managers; they route to Deno.",
    colors::italic("shims")
  );
  if !dir_on_path(&dir) {
    log::info!("");
    log::info!("Add the shim directory to your PATH to start using them:");
    log::info!("");
    if cfg!(windows) {
      log::info!(
        "  {}",
        colors::gray(format!("$env:PATH = \"{};$env:PATH\"", dir.display()))
      );
    } else {
      log::info!(
        "  {}",
        colors::gray(format!("export PATH=\"{}:$PATH\"", dir.display()))
      );
    }
  }
  log::info!("");
  log::info!(
    "Run {} to remove them.",
    colors::bold("deno shim --uninstall")
  );
  Ok(0)
}

fn uninstall() -> Result<i32, AnyError> {
  let dir = shim_dir()?;
  let mut removed = Vec::new();
  for pm in SHIMMED_PMS {
    if remove_shim(&dir, pm)? {
      removed.push(*pm);
    }
  }
  // Drop the one-time notice marker too.
  let _ = std::fs::remove_file(dir.join(NOTICE_MARKER));
  // Remove the directory if it is now empty.
  if dir.exists()
    && std::fs::read_dir(&dir)
      .map(|mut entries| entries.next().is_none())
      .unwrap_or(false)
  {
    let _ = std::fs::remove_dir(&dir);
  }

  if removed.is_empty() {
    log::info!("No shims were installed in {}", dir.display());
  } else {
    log::info!(
      "{} Removed {} shims from {}",
      colors::green("✓"),
      removed.join(", "),
      colors::cyan(dir.to_string_lossy()),
    );
  }
  Ok(0)
}

fn list() -> Result<i32, AnyError> {
  let dir = shim_dir()?;
  log::info!("Shim directory: {}", colors::cyan(dir.to_string_lossy()));
  let on_path = dir_on_path(&dir);
  log::info!(
    "On PATH: {}",
    if on_path {
      colors::green("yes").to_string()
    } else {
      colors::yellow("no").to_string()
    }
  );
  log::info!("");
  for pm in SHIMMED_PMS {
    if shim_installed(&dir, pm) {
      log::info!("  {} {}", colors::green("✓"), pm);
    } else {
      log::info!("  {} {}", colors::gray("·"), colors::gray(*pm));
    }
  }
  if !on_path && SHIMMED_PMS.iter().any(|pm| shim_installed(&dir, pm)) {
    log::info!("");
    log::info!(
      "The shim directory is not on your PATH, so the shims are inactive."
    );
  }
  Ok(0)
}

fn run(pm: &str, args: &[String]) -> Result<i32, AnyError> {
  maybe_print_notice();
  match translate(pm, args) {
    Translation::Deno(deno_args) => exec_deno(&deno_args),
    Translation::Passthrough => exec_real_pm(pm, args),
  }
}

const NOTICE_MARKER: &str = ".shim-notice-shown";

/// Print a one-time notice (per shim directory) clarifying these are shims.
fn maybe_print_notice() {
  let Ok(dir) = shim_dir() else {
    return;
  };
  let marker = dir.join(NOTICE_MARKER);
  if marker.exists() {
    return;
  }
  log::warn!(
    "{}",
    colors::italic_gray(
      "Note: this is a Deno shim, not the real package manager. \
       Unrecognized commands are forwarded to the real binary if available. \
       Run `deno shim --uninstall` to remove these shims."
    )
  );
  let _ = std::fs::create_dir_all(&dir);
  let _ = std::fs::write(&marker, b"");
}

// === translation ===========================================================

#[derive(Debug, PartialEq, Eq)]
enum Translation {
  /// Run the current `deno` binary with these arguments.
  Deno(Vec<String>),
  /// Forward the invocation to the real package manager binary.
  Passthrough,
}

fn translate(pm: &str, args: &[String]) -> Translation {
  match pm {
    "npm" => translate_npm(args),
    "npx" => translate_x(args),
    "pnpm" => translate_pnpm(args),
    "yarn" => translate_yarn(args),
    // Unknown shim name: be safe and pass through.
    _ => Translation::Passthrough,
  }
}

/// Find the first positional token (the package manager subcommand) and its
/// index, skipping nothing — leading flags make the index non-zero.
fn find_subcommand(args: &[String]) -> Option<(usize, &str)> {
  args
    .iter()
    .position(|a| !a.starts_with('-'))
    .map(|i| (i, args[i].as_str()))
}

fn translate_npm(args: &[String]) -> Translation {
  let Some((idx, sub)) = find_subcommand(args) else {
    // Bare `npm` (or only global flags): let the real npm handle it.
    return Translation::Passthrough;
  };
  if idx != 0 {
    // Leading global flags (e.g. `npm --prefix x install`) aren't modeled.
    return Translation::Passthrough;
  }
  let rest = &args[idx + 1..];
  match sub {
    "install" | "i" | "in" | "ins" | "add" | "isntall" => {
      translate_install(rest)
    }
    "ci" | "clean-install" | "install-clean" => translate_ci(rest),
    "uninstall" | "remove" | "rm" | "r" | "un" => translate_remove(rest),
    "run" | "run-script" => translate_run(rest),
    "test" | "start" => translate_lifecycle(sub, rest),
    "exec" => translate_x(rest),
    _ => Translation::Passthrough,
  }
}

fn translate_pnpm(args: &[String]) -> Translation {
  let Some((idx, sub)) = find_subcommand(args) else {
    return Translation::Passthrough;
  };
  if idx != 0 {
    return Translation::Passthrough;
  }
  let rest = &args[idx + 1..];
  match sub {
    "install" | "i" | "add" => translate_install(rest),
    "uninstall" | "remove" | "rm" | "un" => translate_remove(rest),
    "run" | "run-script" => translate_run(rest),
    "test" | "start" => translate_lifecycle(sub, rest),
    "dlx" | "exec" => translate_x(rest),
    _ => Translation::Passthrough,
  }
}

fn translate_yarn(args: &[String]) -> Translation {
  let Some((idx, sub)) = find_subcommand(args) else {
    // Bare `yarn` installs all dependencies.
    return Translation::Deno(vec!["install".to_string()]);
  };
  if idx != 0 {
    return Translation::Passthrough;
  }
  let rest = &args[idx + 1..];
  match sub {
    "install" | "add" => translate_install(rest),
    "remove" => translate_remove(rest),
    "run" => translate_run(rest),
    "test" | "start" => translate_lifecycle(sub, rest),
    "dlx" => translate_x(rest),
    _ => Translation::Passthrough,
  }
}

/// `<pm> install [pkgs...]` → `deno install [--dev] [pkgs...]`.
fn translate_install(rest: &[String]) -> Translation {
  let mut dev = false;
  let mut packages = Vec::new();
  for arg in rest {
    if arg.starts_with('-') {
      match arg.as_str() {
        "-D" | "--save-dev" | "--dev" => dev = true,
        // Save-to-prod flags: this is already Deno's default, so drop them.
        "-S" | "--save" | "--save-prod" | "-P" | "--no-save" => {}
        // Anything else (e.g. `-g`, `--save-exact`, `--save-optional`) has no
        // confident Deno equivalent, so hand the whole thing to the real PM.
        _ => return Translation::Passthrough,
      }
    } else {
      packages.push(arg.clone());
    }
  }
  let mut out = vec!["install".to_string()];
  if dev {
    out.push("--dev".to_string());
  }
  out.extend(packages);
  Translation::Deno(out)
}

/// `<pm> remove <pkgs...>` → `deno remove <pkgs...>`.
fn translate_remove(rest: &[String]) -> Translation {
  let mut packages = Vec::new();
  for arg in rest {
    if arg.starts_with('-') {
      match arg.as_str() {
        "-D" | "--save-dev" | "--dev" | "-S" | "--save" | "--save-prod"
        | "--no-save" => {}
        _ => return Translation::Passthrough,
      }
    } else {
      packages.push(arg.clone());
    }
  }
  if packages.is_empty() {
    return Translation::Passthrough;
  }
  let mut out = vec!["remove".to_string()];
  out.extend(packages);
  Translation::Deno(out)
}

/// `<pm> ci` → `deno ci`. Any extra args aren't modeled → pass through.
fn translate_ci(rest: &[String]) -> Translation {
  if rest.is_empty() {
    Translation::Deno(vec!["ci".to_string()])
  } else {
    Translation::Passthrough
  }
}

/// `<pm> run <task> [args...]` → `deno task <task> [args...]`.
fn translate_run(rest: &[String]) -> Translation {
  // A leading flag before the task name isn't something we model.
  if rest.first().is_some_and(|a| a.starts_with('-')) {
    return Translation::Passthrough;
  }
  let mut out = vec!["task".to_string()];
  out.extend(rest.iter().cloned());
  Translation::Deno(out)
}

/// `<pm> test|start [args...]` → `deno task test|start [args...]`.
fn translate_lifecycle(name: &str, rest: &[String]) -> Translation {
  let mut out = vec!["task".to_string(), name.to_string()];
  out.extend(rest.iter().cloned());
  Translation::Deno(out)
}

/// `npx`/`<pm> exec`/`<pm> dlx [opts] <cmd> [args...]` → `deno x ...`.
fn translate_x(rest: &[String]) -> Translation {
  let mut out = vec!["x".to_string()];
  let mut idx = 0;
  // Consume the small set of recognized leading options.
  while idx < rest.len() {
    match rest[idx].as_str() {
      "--" => {
        idx += 1;
        break;
      }
      "-y" | "--yes" => {
        out.push("-y".to_string());
        idx += 1;
      }
      a if a.starts_with('-') => return Translation::Passthrough,
      // First positional: the command. Stop option parsing.
      _ => break,
    }
  }
  if idx >= rest.len() {
    // No command to run.
    return Translation::Passthrough;
  }
  // Everything from the command onward is forwarded verbatim so the command's
  // own flags are not reinterpreted.
  out.extend(rest[idx..].iter().cloned());
  Translation::Deno(out)
}

// === execution ==============================================================

fn exec_deno(args: &[String]) -> Result<i32, AnyError> {
  let exe = env::current_exe().context("Resolving the deno executable path")?;
  let mut command = std::process::Command::new(exe);
  command.args(args);
  exec_replacing(command)
}

fn exec_real_pm(pm: &str, args: &[String]) -> Result<i32, AnyError> {
  let Some(real) = find_real_binary(pm)? else {
    bail!(
      "`{pm}` is not translated by the Deno shim and no real `{pm}` was found \
       on your PATH.\n  Install {pm}, or run the Deno equivalent directly."
    );
  };
  let mut command = std::process::Command::new(real);
  command.args(args);
  exec_replacing(command)
}

/// Replace the current process with `command` on Unix; on other platforms spawn
/// it and propagate the exit code.
fn exec_replacing(mut command: std::process::Command) -> Result<i32, AnyError> {
  #[cfg(unix)]
  {
    use std::os::unix::process::CommandExt;
    // `exec` only returns if it failed to launch the process.
    Err(command.exec().into())
  }
  #[cfg(not(unix))]
  {
    let mut child = command.spawn().context("Failed to spawn command")?;
    let status = child.wait().context("Failed to wait for command")?;
    Ok(status.code().unwrap_or(1))
  }
}

/// Search `PATH` for the real package manager binary, skipping our own shim
/// directory so we don't recurse into ourselves.
fn find_real_binary(pm: &str) -> Result<Option<PathBuf>, AnyError> {
  let shim_dir = shim_dir().ok().and_then(|d| canonicalize(&d));
  let Some(path) = env::var_os("PATH") else {
    return Ok(None);
  };
  for dir in env::split_paths(&path) {
    if let Some(shim_dir) = &shim_dir
      && canonicalize(&dir).as_ref() == Some(shim_dir)
    {
      continue;
    }
    for candidate in binary_candidates(pm) {
      let full = dir.join(&candidate);
      if full.is_file() {
        return Ok(Some(full));
      }
    }
  }
  Ok(None)
}

fn binary_candidates(pm: &str) -> Vec<String> {
  if cfg!(windows) {
    // Match the executable extensions a shell would resolve.
    ["", ".cmd", ".exe", ".bat", ".ps1"]
      .iter()
      .map(|ext| format!("{pm}{ext}"))
      .collect()
  } else {
    vec![pm.to_string()]
  }
}

fn canonicalize(path: &Path) -> Option<PathBuf> {
  crate::util::fs::canonicalize_path(path).ok()
}

/// Whether `dir` is present on the current `PATH`.
fn dir_on_path(dir: &Path) -> bool {
  let canonical = canonicalize(dir);
  let Some(path) = env::var_os("PATH") else {
    return false;
  };
  env::split_paths(&path).any(|entry| {
    entry == dir || (canonical.is_some() && canonicalize(&entry) == canonical)
  })
}

// === shim files =============================================================

/// Names of the files that make up a shim for `pm` in `dir`.
fn shim_files(dir: &Path, pm: &str) -> Vec<PathBuf> {
  if cfg!(windows) {
    vec![dir.join(format!("{pm}.cmd")), dir.join(format!("{pm}.ps1"))]
  } else {
    vec![dir.join(pm)]
  }
}

fn shim_installed(dir: &Path, pm: &str) -> bool {
  shim_files(dir, pm).iter().any(|p| p.exists())
}

fn write_shim(dir: &Path, pm: &str, exe: &Path) -> Result<(), AnyError> {
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let out_path = dir.join(pm);
    let script = format!(
      "#!/bin/sh\nexec {} shim --run {} -- \"$@\"\n",
      shell_quote(&exe.to_string_lossy()),
      pm,
    );
    std::fs::write(&out_path, script.as_bytes())
      .with_context(|| format!("Writing shim '{}'", out_path.display()))?;
    let mut perms = std::fs::metadata(&out_path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&out_path, perms)?;
  }

  #[cfg(windows)]
  {
    let exe = exe.display().to_string();
    let cmd_path = dir.join(format!("{pm}.cmd"));
    let cmd = format!(
      "@echo off\r\n\"{exe}\" shim --run {pm} -- %*\r\nexit /b %ERRORLEVEL%\r\n"
    );
    std::fs::write(&cmd_path, cmd)
      .with_context(|| format!("Writing shim '{}'", cmd_path.display()))?;

    let ps1_path = dir.join(format!("{pm}.ps1"));
    let ps1 = format!(
      "#!/usr/bin/env pwsh\r\n& \"{exe}\" shim --run {pm} -- $args\r\nexit $LASTEXITCODE\r\n"
    );
    std::fs::write(&ps1_path, ps1)
      .with_context(|| format!("Writing shim '{}'", ps1_path.display()))?;
  }

  Ok(())
}

/// Returns true if any shim file for `pm` existed and was removed.
fn remove_shim(dir: &Path, pm: &str) -> Result<bool, AnyError> {
  let mut removed = false;
  for path in shim_files(dir, pm) {
    match std::fs::remove_file(&path) {
      Ok(()) => removed = true,
      Err(e) if e.kind() == io::ErrorKind::NotFound => {}
      Err(e) => {
        return Err(e)
          .with_context(|| format!("Removing shim '{}'", path.display()));
      }
    }
  }
  Ok(removed)
}

/// Minimal single-quote shell quoting for the embedded deno path on Unix.
#[cfg(unix)]
fn shell_quote(s: &str) -> String {
  format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn deno(args: &[&str]) -> Translation {
    Translation::Deno(args.iter().map(|s| s.to_string()).collect())
  }

  fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
  }

  #[test]
  fn npm_install() {
    assert_eq!(translate("npm", &args(&["install"])), deno(&["install"]));
    assert_eq!(
      translate("npm", &args(&["install", "express"])),
      deno(&["install", "express"])
    );
    assert_eq!(
      translate("npm", &args(&["i", "express"])),
      deno(&["install", "express"])
    );
    assert_eq!(
      translate("npm", &args(&["install", "-D", "vitest"])),
      deno(&["install", "--dev", "vitest"])
    );
    assert_eq!(
      translate("npm", &args(&["install", "--save-dev", "vitest"])),
      deno(&["install", "--dev", "vitest"])
    );
    // --save is the default, so it's dropped.
    assert_eq!(
      translate("npm", &args(&["install", "--save", "express"])),
      deno(&["install", "express"])
    );
  }

  #[test]
  fn npm_install_unknown_flag_passthrough() {
    // -g (global package install) has no confident Deno equivalent.
    assert_eq!(
      translate("npm", &args(&["install", "-g", "typescript"])),
      Translation::Passthrough
    );
    assert_eq!(
      translate("npm", &args(&["install", "--save-exact", "x"])),
      Translation::Passthrough
    );
  }

  #[test]
  fn npm_ci_run_exec() {
    assert_eq!(translate("npm", &args(&["ci"])), deno(&["ci"]));
    assert_eq!(
      translate("npm", &args(&["ci", "--omit=dev"])),
      Translation::Passthrough
    );
    assert_eq!(
      translate("npm", &args(&["run", "build"])),
      deno(&["task", "build"])
    );
    assert_eq!(
      translate("npm", &args(&["run", "build", "--", "--watch"])),
      deno(&["task", "build", "--", "--watch"])
    );
    assert_eq!(
      translate("npm", &args(&["exec", "cowsay", "hi"])),
      deno(&["x", "cowsay", "hi"])
    );
    assert_eq!(translate("npm", &args(&["test"])), deno(&["task", "test"]));
  }

  #[test]
  fn npm_remove() {
    assert_eq!(
      translate("npm", &args(&["uninstall", "express"])),
      deno(&["remove", "express"])
    );
    assert_eq!(
      translate("npm", &args(&["rm", "express"])),
      deno(&["remove", "express"])
    );
    // No package to remove → pass through.
    assert_eq!(
      translate("npm", &args(&["uninstall"])),
      Translation::Passthrough
    );
  }

  #[test]
  fn npx() {
    assert_eq!(
      translate("npx", &args(&["cowsay", "hello"])),
      deno(&["x", "cowsay", "hello"])
    );
    assert_eq!(
      translate("npx", &args(&["-y", "cowsay", "hello"])),
      deno(&["x", "-y", "cowsay", "hello"])
    );
    // `--` separator is consumed.
    assert_eq!(
      translate("npx", &args(&["--", "cowsay"])),
      deno(&["x", "cowsay"])
    );
    // Command flags after the command are forwarded verbatim.
    assert_eq!(
      translate("npx", &args(&["eslint", "--fix", "."])),
      deno(&["x", "eslint", "--fix", "."])
    );
    // Unrecognized leading option → pass through.
    assert_eq!(
      translate("npx", &args(&["--package=foo", "bar"])),
      Translation::Passthrough
    );
    // No command → pass through.
    assert_eq!(translate("npx", &args(&[])), Translation::Passthrough);
  }

  #[test]
  fn pnpm() {
    assert_eq!(
      translate("pnpm", &args(&["add", "-D", "vitest"])),
      deno(&["install", "--dev", "vitest"])
    );
    assert_eq!(
      translate("pnpm", &args(&["dlx", "create-vite"])),
      deno(&["x", "create-vite"])
    );
    assert_eq!(
      translate("pnpm", &args(&["run", "dev"])),
      deno(&["task", "dev"])
    );
  }

  #[test]
  fn yarn() {
    // Bare `yarn` installs everything.
    assert_eq!(translate("yarn", &args(&[])), deno(&["install"]));
    assert_eq!(
      translate("yarn", &args(&["add", "lodash"])),
      deno(&["install", "lodash"])
    );
    assert_eq!(
      translate("yarn", &args(&["dlx", "create-vite"])),
      deno(&["x", "create-vite"])
    );
    assert_eq!(
      translate("yarn", &args(&["run", "build"])),
      deno(&["task", "build"])
    );
  }

  #[test]
  fn unknown_subcommand_passthrough() {
    assert_eq!(
      translate("npm", &args(&["publish"])),
      Translation::Passthrough
    );
    assert_eq!(
      translate("pnpm", &args(&["store", "prune"])),
      Translation::Passthrough
    );
    assert_eq!(
      translate("yarn", &args(&["why", "x"])),
      Translation::Passthrough
    );
  }

  #[test]
  fn leading_global_flag_passthrough() {
    assert_eq!(
      translate("npm", &args(&["--prefix", "/tmp", "install"])),
      Translation::Passthrough
    );
  }
}
