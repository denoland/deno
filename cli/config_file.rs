// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::fs_util::canonicalize_path;
use deno_core::error::AnyError;
use deno_core::serde_json::Value;
use std::path::PathBuf;

pub struct ConfigFile {
  pub path: PathBuf,
  pub value: Value,
}

impl ConfigFile {
  pub fn read(path: &str) -> Result<Self, AnyError> {
    let cwd = std::env::current_dir()?;
    let config_file = cwd.join(path);
    let config_path = canonicalize_path(&config_file).map_err(|_| {
      std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!(
          "Could not find the config file: {}",
          config_file.to_string_lossy()
        ),
      )
    })?;
    let config_text = std::fs::read_to_string(config_path.clone())?;
    let json_value = jsonc_parser::parse_to_serde_value(&config_text)?.unwrap();

    Ok(Self {
      path: config_path,
      value: json_value,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn read_config_file() {
    let config_file = ConfigFile::read("tests/module_graph/tsconfig.json")
      .expect("Failed to load config file");
  }
}
