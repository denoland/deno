// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;

/// Run `git` with `args` in `cwd`, returning stdout on success.
pub fn run_git(cwd: &Path, args: &[&str]) -> Result<String, AnyError> {
  let output = match std::process::Command::new("git")
    .current_dir(cwd)
    .args(args)
    .output()
  {
    Ok(output) => output,
    Err(err) => {
      return Err(anyhow!(
        "Failed to run `git {}`: {err}. Is git installed and on PATH?",
        args.join(" ")
      ));
    }
  };
  if !output.status.success() {
    return Err(anyhow!(
      "`git {}` failed: {}",
      args.join(" "),
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }
  Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
