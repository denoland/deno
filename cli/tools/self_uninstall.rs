// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::IsTerminal;
use std::io::Write;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;

use crate::args::Flags;
use crate::args::SelfUninstallFlags;
use crate::colors;
use crate::factory::CliFactory;

pub async fn self_uninstall(
  flags: Arc<Flags>,
  self_uninstall_flags: SelfUninstallFlags,
) -> Result<(), AnyError> {
  let current_exe_path = std::env::current_exe()
    .context("Failed to get the path of the current executable")?;

  let factory = CliFactory::from_flags(flags);
  let deno_dir = factory.deno_dir()?;
  let cache_dir = deno_dir.root.clone();

  log::info!(
    "This will remove the Deno executable at {} and the cache directory at {}.",
    colors::bold(current_exe_path.display()),
    colors::bold(cache_dir.display()),
  );

  if !self_uninstall_flags.yes {
    if !std::io::stdin().is_terminal() {
      bail!(
        "Cannot prompt for confirmation in non-interactive mode. Use the --yes flag to skip confirmation."
      );
    }

    eprint!("Do you want to continue? [y/N] ");
    std::io::stderr().flush().ok();
    let mut input = String::new();
    std::io::stdin()
      .read_line(&mut input)
      .context("Failed to read user input")?;

    if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
      log::info!("Uninstall aborted.");
      return Ok(());
    }
  }

  // Remove cache directory first
  if cache_dir.exists() {
    std::fs::remove_dir_all(&cache_dir).with_context(|| {
      format!(
        "Failed to remove cache directory: {}",
        cache_dir.display()
      )
    })?;
    log::info!("{} {}", colors::green("Removed"), cache_dir.display());
  }

  // Remove the Deno binary
  std::fs::remove_file(&current_exe_path).with_context(|| {
    format!(
      "Failed to remove Deno executable: {}. You may need to remove it manually.",
      current_exe_path.display()
    )
  })?;
  log::info!(
    "{} {}",
    colors::green("Removed"),
    current_exe_path.display()
  );

  log::info!("✅ Deno has been uninstalled.");
  Ok(())
}
