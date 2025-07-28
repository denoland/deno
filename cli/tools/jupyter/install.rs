// Copyright 2018-2025 the Deno authors. MIT license.

use std::env::current_exe;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;

static TEST_ENV_VAR_NAME: &str = "DENO_TEST_JUPYTER_PATH";

const DENO_ICON_32: &[u8] = include_bytes!("./resources/deno-logo-32x32.png");
const DENO_ICON_64: &[u8] = include_bytes!("./resources/deno-logo-64x64.png");
const DENO_ICON_SVG: &[u8] = include_bytes!("./resources/deno-logo-svg.svg");

fn get_user_data_dir() -> Result<PathBuf, AnyError> {
  Ok(if let Some(env_var) = std::env::var_os(TEST_ENV_VAR_NAME) {
    PathBuf::from(env_var)
  } else {
    jupyter_runtime::dirs::user_data_dir()?
  })
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
  let current_exe_path = current_exe()
    .context("Failed to get current executable path")?
    .to_string_lossy()
    .into_owned();

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
