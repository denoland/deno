use crate::npm::managed::NpmResolutionPackage;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use std::fs;
use std::path::Path;

pub(super) fn set_up_bin_entry(
  package: &NpmResolutionPackage,
  bin_name: &str,
  #[allow(unused_variables)] bin_script: &str,
  #[allow(unused_variables)] package_path: &Path,
  bin_node_modules_dir_path: &Path,
) -> Result<(), AnyError> {
  #[cfg(windows)]
  {
    set_up_bin_shim(package, bin_name, bin_node_modules_dir_path)?;
  }
  #[cfg(unix)]
  {
    symlink_bin_entry(
      package,
      bin_name,
      bin_script,
      package_path,
      bin_node_modules_dir_path,
    )?;
  }
  Ok(())
}

#[cfg(windows)]
fn set_up_bin_shim(
  package: &NpmResolutionPackage,
  bin_name: &str,
  bin_node_modules_dir_path: &Path,
) -> Result<(), AnyError> {
  let mut cmd_shim = bin_node_modules_dir_path.join(bin_name);

  cmd_shim.set_extension("cmd");
  let shim = format!("@deno run -A npm:{}/{bin_name} %*", package.id.nv);
  if cmd_shim.exists() {
    if let Ok(contents) = fs::read_to_string(cmd_shim) {
      if contents == shim {
        // up to date
        return Ok(());
      }
    }
    return Ok(());
  }
  fs::write(&cmd_shim, shim).with_context(|| {
    format!("Can't set up '{}' bin at {}", bin_name, cmd_shim.display())
  })?;

  Ok(())
}

#[cfg(unix)]
fn symlink_bin_entry(
  package: &NpmResolutionPackage,
  bin_name: &str,
  bin_script: &str,
  package_path: &Path,
  bin_node_modules_dir_path: &Path,
) -> Result<(), AnyError> {
  use std::os::unix::fs::symlink;
  let link = bin_node_modules_dir_path.join(bin_name);
  let original = package_path.join(bin_script);

  // Don't bother setting up another link if it already exists
  if link.exists() {
    let resolved = std::fs::read_link(&link).ok();
    if let Some(resolved) = resolved {
      if resolved != original {
        log::warn!(
          "{} Trying to set up '{}' bin for \"{}\", but an entry pointing to \"{}\" already exists. Skipping...", 
          deno_terminal::colors::yellow("Warning"), 
          bin_name,
          resolved.display(),
          original.display()
        );
      }
      return Ok(());
    }
  }

  use std::os::unix::fs::PermissionsExt;
  let mut perms = std::fs::metadata(&original).unwrap().permissions();
  if perms.mode() & 0o111 == 0 {
    // if the original file is not executable, make it executable
    perms.set_mode(perms.mode() | 0o111);
    std::fs::set_permissions(&original, perms).with_context(|| {
      format!("Setting permissions on '{}'", original.display())
    })?;
  }
  let original_relative =
    pathdiff::diff_paths(&original, bin_node_modules_dir_path)
      .unwrap_or(original);
  symlink(&original_relative, &link).with_context(|| {
    format!(
      "Can't set up '{}' bin at {}",
      bin_name,
      original_relative.display()
    )
  })?;

  Ok(())
}
