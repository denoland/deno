// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use std::env::current_exe;
use std::io::Write;
use std::path::Path;

use jupyter_runtime::dirs::user_data_dir;

const DENO_ICON_32: &[u8] = include_bytes!("./resources/deno-logo-32x32.png");
const DENO_ICON_64: &[u8] = include_bytes!("./resources/deno-logo-64x64.png");
const DENO_ICON_SVG: &[u8] = include_bytes!("./resources/deno-logo-svg.svg");

pub fn status() -> Result<(), AnyError> {
  let user_data_dir = user_data_dir()?;

  let kernel_spec_dir_path = user_data_dir.join("kernels").join("deno");
  let kernel_spec_path = kernel_spec_dir_path.join("kernel.json");

  if kernel_spec_path.exists() {
    log::info!("✅ Deno kernel already installed");
    Ok(())
  } else {
    log::warn!("ℹ️ Deno kernel is not yet installed, run `deno jupyter --install` to set it up");
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

pub fn install() -> Result<(), AnyError> {
  let user_data_dir = user_data_dir()?;
  let kernel_dir = user_data_dir.join("kernels").join("deno");

  std::fs::create_dir_all(&kernel_dir)?;

  let kernel_json_path = kernel_dir.join("kernel.json");

  // TODO(bartlomieju): add remaining fields as per
  // https://jupyter-client.readthedocs.io/en/stable/kernels.html#kernel-specs
  // FIXME(bartlomieju): replace `current_exe` before landing?
  let json_data = json!({
      "argv": [current_exe().unwrap().to_string_lossy(), "jupyter", "--kernel", "--conn", "{connection_file}"],
      "display_name": "Deno",
      "language": "typescript",
  });

  let f = std::fs::File::create(kernel_json_path)?;
  serde_json::to_writer_pretty(f, &json_data)?;
  install_icon(&kernel_dir, "logo-32x32.png", DENO_ICON_32)?;
  install_icon(&kernel_dir, "logo-64x64.png", DENO_ICON_64)?;
  install_icon(&kernel_dir, "logo-svg.svg", DENO_ICON_SVG)?;

  log::info!("✅ Deno kernelspec installed successfully.");
  Ok(())
}
