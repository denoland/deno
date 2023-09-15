// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use std::env::current_exe;
use tempfile::TempDir;

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

  println!("ℹ️ Deno kernel is not yet installed, run `deno jupyter --unstable --install` to set it up");
  Ok(())
}

pub fn install() -> Result<(), AnyError> {
  let temp_dir = TempDir::new().unwrap();
  let kernel_json_path = temp_dir.path().join("kernel.json");

  // TODO(bartlomieju): add remaining fields as per
  // https://jupyter-client.readthedocs.io/en/stable/kernels.html#kernel-specs
  // FIXME(bartlomieju): replace `current_exe` before landing?
  let json_data = json!({
      "argv": [current_exe().unwrap().to_string_lossy(), "--unstable", "jupyter", "--kernel", "--conn", "{connection_file}"],
      "display_name": "Deno",
      "language": "typescript",
  });

  let f = std::fs::File::create(kernel_json_path)?;
  serde_json::to_writer_pretty(f, &json_data)?;

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

  // TODO(bartlomieju): copy icons the the kernelspec directory

  if let Ok(mut child) = child_result {
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
  }

  let _ = std::fs::remove_dir(temp_dir);
  println!("Deno kernelspec installed successfully.");
  Ok(())
}
