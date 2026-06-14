// Copyright 2018-2026 the Deno authors. MIT license.

use std::env::current_exe;
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;

use crate::util::fs::canonicalize_path;

static TEST_ENV_VAR_NAME: &str = "DENO_TEST_JUPYTER_PATH";

const DENO_ICON_32: &[u8] = include_bytes!("./resources/deno-logo-32x32.png");
const DENO_ICON_64: &[u8] = include_bytes!("./resources/deno-logo-64x64.png");
const DENO_ICON_SVG: &[u8] = include_bytes!("./resources/deno-logo-svg.svg");

/// Resolves the most stable path to the current Deno executable to embed in
/// the generated Jupyter kernelspec's `argv`.
///
/// `std::env::current_exe()` resolves symlinks. For package-manager installs –
/// most notably Homebrew, which symlinks `/opt/homebrew/bin/deno` to a
/// versioned path under `Cellar/deno/<version>/bin/deno` – the resolved path
/// embeds the installed version number and stops existing after an upgrade,
/// leaving the kernel pointing at a missing binary (`spawn ... ENOENT`, see
/// https://github.com/denoland/deno/issues/25306).
///
/// To keep the kernel working across upgrades we prefer a `deno` entry found on
/// `PATH` that resolves to the same binary (e.g. `/opt/homebrew/bin/deno`),
/// falling back to the resolved executable path when no such entry exists.
fn kernel_exe_path() -> Result<PathBuf, AnyError> {
  let current_exe =
    current_exe().context("Failed to get current executable path")?;
  Ok(stable_exe_path(&current_exe, std::env::var_os("PATH")))
}

fn stable_exe_path(current_exe: &Path, path_var: Option<OsString>) -> PathBuf {
  // Canonicalize the running binary so it can be compared against the resolved
  // target of each `PATH` candidate.
  let Ok(target) = canonicalize_path(current_exe) else {
    return current_exe.to_path_buf();
  };
  let (Some(file_name), Some(path_var)) = (current_exe.file_name(), path_var)
  else {
    return current_exe.to_path_buf();
  };
  for dir in std::env::split_paths(&path_var) {
    if dir.as_os_str().is_empty() {
      continue;
    }
    let candidate = dir.join(file_name);
    if canonicalize_path(&candidate).is_ok_and(|c| c == target) {
      return candidate;
    }
  }
  current_exe.to_path_buf()
}

fn get_user_data_dir() -> Result<PathBuf, AnyError> {
  if let Some(env_var) = std::env::var_os(TEST_ENV_VAR_NAME) {
    return Ok(PathBuf::from(env_var));
  }
  // Platform-specific Jupyter user data directory (mirrors runtimelib behavior).
  #[cfg(target_os = "macos")]
  {
    let home = std::env::var_os("HOME")
      .map(PathBuf::from)
      .ok_or_else(|| deno_core::anyhow::anyhow!("HOME not set"))?;
    Ok(home.join("Library").join("Jupyter"))
  }
  #[cfg(target_os = "windows")]
  {
    let appdata = std::env::var_os("APPDATA")
      .map(PathBuf::from)
      .ok_or_else(|| deno_core::anyhow::anyhow!("APPDATA not set"))?;
    Ok(appdata.join("jupyter"))
  }
  #[cfg(not(any(target_os = "macos", target_os = "windows")))]
  {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
      return Ok(PathBuf::from(xdg).join("jupyter"));
    }
    let home = std::env::var_os("HOME")
      .map(PathBuf::from)
      .ok_or_else(|| deno_core::anyhow::anyhow!("HOME not set"))?;
    Ok(home.join(".local").join("share").join("jupyter"))
  }
}

pub fn status(maybe_name: Option<&str>) -> Result<(), AnyError> {
  let user_data_dir = get_user_data_dir()?;

  let kernel_name = maybe_name.unwrap_or("deno");
  let kernel_spec_dir_path = user_data_dir.join("kernels").join(kernel_name);
  let kernel_spec_path = kernel_spec_dir_path.join("kernel.json");

  if kernel_spec_path.exists() {
    log::info!(
      "✅ Deno kernel already installed at {}",
      kernel_spec_dir_path.display()
    );
    Ok(())
  } else {
    let mut install_cmd = "deno jupyter --install".to_string();
    if let Some(name) = maybe_name {
      install_cmd.push_str(" --name ");
      install_cmd.push_str(name);
    }
    log::warn!(
      "ℹ️ Deno kernel is not yet installed, run `{}` to set it up",
      install_cmd
    );
    Ok(())
  }
}

fn install_icon(
  dir_path: &Path,
  filename: &str,
  icon_data: &[u8],
) -> Result<(), AnyError> {
  let path = dir_path.join(filename);
  let mut file = std::fs::File::create(path)?;
  file.write_all(icon_data)?;
  Ok(())
}

pub fn install(
  maybe_name: Option<&str>,
  maybe_display_name: Option<&str>,
  force: bool,
) -> Result<(), AnyError> {
  let user_data_dir = get_user_data_dir()?;

  let kernel_name = maybe_name.unwrap_or("deno");
  let kernel_spec_dir_path = user_data_dir.join("kernels").join(kernel_name);
  let kernel_spec_path = kernel_spec_dir_path.join("kernel.json");

  std::fs::create_dir_all(&kernel_spec_dir_path).with_context(|| {
    format!(
      "Failed to create kernel directory at {}",
      kernel_spec_dir_path.display()
    )
  })?;

  if kernel_spec_path.exists() && !force {
    bail!(
      "Deno kernel already exists at {}, run again with `--force` to overwrite it",
      kernel_spec_dir_path.display()
    );
  }

  let display_name = maybe_display_name.unwrap_or("Deno");
  let current_exe_path = kernel_exe_path()?.to_string_lossy().into_owned();

  // TODO(bartlomieju): add remaining fields as per
  // https://jupyter-client.readthedocs.io/en/stable/kernels.html#kernel-specs
  let json_data = json!({
      "argv": [current_exe_path, "jupyter", "--kernel", "--conn", "{connection_file}"],
      "display_name": display_name,
      "language": "typescript",
  });

  let f = std::fs::File::create(&kernel_spec_path).with_context(|| {
    format!(
      "Failed to create kernelspec file at {}",
      kernel_spec_path.display()
    )
  })?;
  serde_json::to_writer_pretty(f, &json_data).with_context(|| {
    format!(
      "Failed to write kernelspec file at {}",
      kernel_spec_path.display()
    )
  })?;
  let failed_icon_fn =
    || format!("Failed to copy icon to {}", kernel_spec_dir_path.display());
  install_icon(&kernel_spec_dir_path, "logo-32x32.png", DENO_ICON_32)
    .with_context(failed_icon_fn)?;
  install_icon(&kernel_spec_dir_path, "logo-64x64.png", DENO_ICON_64)
    .with_context(failed_icon_fn)?;
  install_icon(&kernel_spec_dir_path, "logo-svg.svg", DENO_ICON_SVG)
    .with_context(failed_icon_fn)?;

  log::info!(
    "✅ Deno kernelspec installed successfully at {}.",
    kernel_spec_dir_path.display()
  );
  Ok(())
}

#[cfg(all(test, unix))]
mod tests {
  use std::os::unix::fs::symlink;

  use super::*;

  #[test]
  fn stable_exe_path_prefers_path_symlink() {
    // Simulates a Homebrew install: the running binary is the resolved,
    // versioned `Cellar` path, while `PATH` contains a stable symlink to it.
    let tmp = tempfile::tempdir().unwrap();
    let cellar_dir = tmp.path().join("Cellar").join("deno").join("1.0.0");
    let bin_dir = tmp.path().join("bin");
    std::fs::create_dir_all(&cellar_dir).unwrap();
    std::fs::create_dir_all(&bin_dir).unwrap();

    let resolved_exe = cellar_dir.join("deno");
    std::fs::write(&resolved_exe, b"").unwrap();
    let symlinked_exe = bin_dir.join("deno");
    symlink(&resolved_exe, &symlinked_exe).unwrap();

    let path_var = OsString::from(bin_dir.to_string_lossy().into_owned());
    assert_eq!(
      stable_exe_path(&resolved_exe, Some(path_var)),
      symlinked_exe
    );
  }

  #[test]
  fn stable_exe_path_without_match_falls_back() {
    let tmp = tempfile::tempdir().unwrap();
    let resolved_exe = tmp.path().join("deno");
    std::fs::write(&resolved_exe, b"").unwrap();

    // No PATH at all.
    assert_eq!(stable_exe_path(&resolved_exe, None), resolved_exe);

    // Empty PATH entries are ignored.
    assert_eq!(
      stable_exe_path(&resolved_exe, Some(OsString::from(""))),
      resolved_exe
    );

    // A PATH that doesn't contain a matching `deno` falls back to the exe.
    let other_dir = tmp.path().join("other");
    std::fs::create_dir_all(&other_dir).unwrap();
    assert_eq!(
      stable_exe_path(
        &resolved_exe,
        Some(OsString::from(other_dir.to_string_lossy().into_owned()))
      ),
      resolved_exe
    );
  }

  #[test]
  fn stable_exe_path_ignores_unrelated_path_deno() {
    // A different `deno` binary on PATH that doesn't resolve to the running
    // executable must not be chosen.
    let tmp = tempfile::tempdir().unwrap();
    let real_exe = tmp.path().join("real-deno");
    std::fs::write(&real_exe, b"").unwrap();

    let bin_dir = tmp.path().join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::write(bin_dir.join("deno"), b"").unwrap();

    let path_var = OsString::from(bin_dir.to_string_lossy().into_owned());
    // `real_exe`'s file name is `real-deno`, so no candidate matches; even if
    // it were `deno`, the unrelated binary canonicalizes elsewhere.
    assert_eq!(stable_exe_path(&real_exe, Some(path_var)), real_exe);
  }
}
