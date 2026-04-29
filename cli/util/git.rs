// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

pub async fn check_if_git_repo_dirty(cwd: &Path) -> Option<String> {
  let bin_name = if cfg!(windows) { "git.exe" } else { "git" };

  //  Check if git exists
  let git_exists = Command::new(bin_name)
    .arg("--version")
    .stderr(Stdio::null())
    .stdout(Stdio::null())
    .status()
    .await
    .is_ok_and(|status| status.success());

  if !git_exists {
    return None; // Git is not installed
  }

  // Check if there are uncommitted changes. If git fails (killed,
  // permissions, repo missing), treat it the same as "git not installed":
  // we just don't have a working-tree signal, so don't block the user.
  let Ok(output) = Command::new(bin_name)
    .current_dir(cwd)
    .args(["status", "--porcelain"])
    .output()
    .await
  else {
    return None;
  };

  let output_str = String::from_utf8_lossy(&output.stdout);
  let text = output_str.trim();
  if text.is_empty() {
    None
  } else {
    Some(text.to_string())
  }
}
