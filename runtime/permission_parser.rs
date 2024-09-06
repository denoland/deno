// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::normalize_path;
use deno_permissions::EnvDescriptor;
use deno_permissions::NetDescriptor;
use deno_permissions::ReadDescriptor;
use deno_permissions::WriteDescriptor;

pub struct RuntimePermissionParser {
  initial_cwd: PathBuf,
  fs: deno_fs::FileSystemRc,
}

impl RuntimePermissionParser {
  fn resolve_from_cwd(&self, path: &Path) -> PathBuf {
    if path.is_absolute() {
      normalize_path(path)
    } else {
      normalize_path(self.initial_cwd.join(path))
    }
  }

  fn resolve_path(
    &self,
    text: &str,
  ) -> Result<PathBuf, deno_core::anyhow::Error> {
    if text.is_empty() {
      Err(AnyError::msg("Empty path is not allowed"))
    } else {
      Ok(self.resolve_from_cwd(Path::new(text)))
    }
  }
}

impl deno_permissions::PermissionParser for RuntimePermissionParser {
  fn parse_read_descriptor(
    &self,
    text: &str,
  ) -> Result<ReadDescriptor, AnyError> {
    Ok(ReadDescriptor(self.resolve_path(text)?))
  }

  fn parse_write_descriptor(
    &self,
    text: &str,
  ) -> Result<WriteDescriptor, AnyError> {
    Ok(WriteDescriptor(self.resolve_path(text)?))
  }

  fn parse_net_descriptor(
    &self,
    text: &str,
  ) -> Result<NetDescriptor, AnyError> {
    NetDescriptor::parse(text)
  }

  fn parse_env_descriptor(
    &self,
    text: &str,
  ) -> Result<EnvDescriptor, AnyError> {
    if x.is_empty() {
      Err(AnyError::msg("Empty path is not allowed"))
    } else {
      Ok(EnvDescriptor::new(x))
    }
  }
}
