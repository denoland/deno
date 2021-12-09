// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::flags::Flags;
use crate::flags::JupyterFlags;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use std::env::current_exe;
use tempfile::TempDir;

pub fn install() -> Result<(), AnyError> {
  let temp_dir = TempDir::new().unwrap();
  let kernel_json_path = temp_dir.path().join("kernel.json");

  // TODO(bartlomieju): add remaining fields as per
  // https://jupyter-client.readthedocs.io/en/stable/kernels.html#kernel-specs
  // FIXME(bartlomieju): replace `current_exe`
  let json_data = json!({
      "argv": [current_exe().unwrap().to_string_lossy(), "jupyter", "--conn", "{connection_file}"],
      "display_name": "Deno",
      "language": "typescript",
  });

  let f = std::fs::File::create(kernel_json_path)?;
  serde_json::to_writer_pretty(f, &json_data)?;

  let child_result = std::process::Command::new("jupyter")
    .args([
      "kernelspec",
      "install",
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
          eprintln!("Failed to install kernelspec, try again.");
        }
      }
      Err(err) => {
        eprintln!("Failed to install kernelspec: {}", err);
      }
    }
  }

  let _ = std::fs::remove_dir(temp_dir);
  println!("Deno kernelspec installed successfully.");
  Ok(())
}

pub async fn kernel(
  _flags: Flags,
  jupyter_flags: JupyterFlags,
) -> Result<(), AnyError> {
  let conn_file_path = jupyter_flags.conn_file.unwrap();

  let conn_file = std::fs::read_to_string(conn_file_path)
    .context("Failed to read connection file")?;
  let conn_spec: ConnectionSpec = serde_json::from_str(&conn_file)
    .context("Failed to parse connection file")?;
  eprintln!("[DENO] parsed conn file: {:#?}", conn_spec);

  let kernel = Kernel {
    metadata: KernelMetadata::default(),
    conn_spec,
    state: KernelState::Idle,
  };

  eprintln!("[DENO] kernel created: {:#?}", kernel);

  Ok(())
}

#[derive(Debug)]
enum KernelState {
  Busy,
  Idle,
}

#[derive(Debug)]
struct Kernel {
  metadata: KernelMetadata,
  conn_spec: ConnectionSpec,
  state: KernelState,
}

#[derive(Debug)]
struct KernelMetadata {
  banner: String,
  file_ext: String,
  help_text: String,
  help_url: String,
  implementation_name: String,
  kernel_version: String,
  language_version: String,
  language: String,
  mime: String,
  protocol_version: String,
  session_id: String,
}

impl Default for KernelMetadata {
  fn default() -> Self {
    Self {
      banner: "Welcome to Deno kernel".to_string(),
      file_ext: ".ts".to_string(),
      help_text: "<TODO>".to_string(),
      help_url: "https://github.com/denoland/deno".to_string(),
      implementation_name: "Deno kernel".to_string(),
      // FIXME:
      kernel_version: "0.0.1".to_string(),
      // FIXME:
      language_version: "1.16.4".to_string(),
      language: "typescript".to_string(),
      // FIXME:
      mime: "text/x.typescript".to_string(),
      protocol_version: "5.3".to_string(),
      session_id: uuid::Uuid::new_v4().to_string(),
    }
  }
}

#[derive(Debug, Deserialize)]
struct ConnectionSpec {
  ip: String,
  transport: String,
  control_port: u32,
  shell_port: u32,
  stdin_port: u32,
  hb_port: u32,
  iopub_port: u32,
  signature_scheme: String,
  key: String,
}
