// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use std::env::current_exe;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;

const DENO_ICON_32: &[u8] = include_bytes!("./resources/deno-logo-32x32.png");
const DENO_ICON_64: &[u8] = include_bytes!("./resources/deno-logo-64x64.png");
const DENO_ICON_SVG: &[u8] = include_bytes!("./resources/deno-logo-svg.svg");

pub fn status() -> Result<(), AnyError> {
  let output = std::process::Command::new("jupyter")
    .args(["kernelspec", "list", "--json"])
    .output()
    .context("Failed to get list of installed kernelspecs")?;
  let json_output: serde_json::Value =
    serde_json::from_slice(&output.stdout)
      .context("Failed to parse JSON from kernelspec list")?;

  if let Some(specs) = json_output.get("kernelspecs") {
    if let Some(specs_obj) = specs.as_object() {
      if specs_obj.contains_key("deno") {
        println!("✅ Deno kernel already installed");
        return Ok(());
      }
    }
  }

  println!("ℹ️ Deno kernel is not yet installed, run `deno jupyter --install` to set it up");
  Ok(())
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

pub fn install(directory: Option<PathBuf>) -> Result<(), AnyError> {
  let outcome = match directory {
    Some(path) => install_via_directory_flag(&path),
    None => install_via_jupyter(),
  };

  if outcome.is_err() {
    bail!("🆘 Failed to install Deno kernel: {:?}", outcome.unwrap_err());
  }

  println!("✅ Deno kernelspec installed successfully.");
  Ok(())
}

fn install_via_directory_flag(output_dir: &PathBuf) -> Result<(), AnyError> {
  let mut dir = output_dir.clone();
  if !dir.ends_with("deno") {
    dir.push("deno");
  }

  std::fs::create_dir_all(dir.as_path())?;
  create_kernelspec_in_directory(&dir)?;

  log::info!("📂 Installing to custom location: {}.", &dir.display());
  Ok(())
}

fn create_kernelspec_in_directory(directory: &PathBuf) -> Result<(), AnyError> {
  let kernel_json_path = directory.join("kernel.json");
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
  install_icon(directory, "logo-32x32.png", DENO_ICON_32)?;
  install_icon(directory, "logo-64x64.png", DENO_ICON_64)?;
  install_icon(directory, "logo-svg.svg", DENO_ICON_SVG)?;
  Ok(())
}

fn install_via_jupyter() -> Result<(), AnyError> {
  let temp_dir = TempDir::new().unwrap();

  let create_kernelspec_result =
    create_kernelspec_in_directory(&temp_dir.path().to_path_buf());
  if create_kernelspec_result.is_err() {
    return create_kernelspec_result;
  }

  let child_result = std::process::Command::new("jupyter")
    .args([
      "kernelspec",
      "install",
      "--user",
      "--name",
      "deno",
      &temp_dir.path().to_string_lossy(),
    ])
    .spawn();
  let mut child = match child_result {
    Ok(child) => child,
    Err(err)
      if matches!(
        err.kind(),
        ErrorKind::NotFound | ErrorKind::PermissionDenied
      ) =>
    {
      return Err(err).context(concat!(
        "Failed to spawn 'jupyter' command. Is JupyterLab installed ",
        "(https://jupyter.org/install) and available on the PATH?"
      ));
    }
    Err(err) => {
      return Err(err).context("Failed to spawn 'jupyter' command.");
    }
  };

  let wait_result = child.wait();
  match wait_result {
    Ok(status) => {
      if !status.success() {
        bail!("Failed to install kernelspec, try again.");
      }
    }
    Err(err) => {
      bail!("Failed to install kernelspec: {}", err);
    }
  }
  let _ = std::fs::remove_dir(temp_dir);
  Ok(())
}
