// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::normalize_path;
use deno_permissions::AllowRunDescriptor;
use deno_permissions::DenyRunDescriptor;
use deno_permissions::EnvDescriptor;
use deno_permissions::FfiDescriptor;
use deno_permissions::NetDescriptor;
use deno_permissions::ReadDescriptor;
use deno_permissions::SysDescriptor;
use deno_permissions::WriteDescriptor;

pub struct RuntimePermissionDescriptorParser {
  fs: deno_fs::FileSystemRc,
  initial_cwd: Option<PathBuf>,
}

impl RuntimePermissionDescriptorParser {
  pub fn new(fs: deno_fs::FileSystemRc, initial_cwd: Option<PathBuf>) {
    Self { fs, initial_cwd }
  }

  fn initial_cwd(&self) -> Result<&Path, AnyError> {
    if let Some(initial_cwd) = &self.initial_cwd {
      Ok(initial_cwd)
    } else {
      bail!("Could not resolve permission path when current working directory could not be resolved.")
    }
  }

  fn resolve_from_cwd(&self, path: &Path) -> Result<PathBuf, AnyError> {
    if path.is_absolute() {
      Ok(normalize_path(path))
    } else if let Some(initial_cwd) = &self.initial_cwd {
      Ok(normalize_path(initial_cwd.join(path)))
    } else {
      bail!("Could not resolve relative permission path '{}' when current working directory could not be resolved.", path.display())
    }
  }

  fn resolve_path(
    &self,
    text: &str,
  ) -> Result<PathBuf, deno_core::anyhow::Error> {
    if text.is_empty() {
      Err(AnyError::msg("Empty path is not allowed"))
    } else {
      self.resolve_from_cwd(Path::new(text))
    }
  }
}

impl deno_permissions::PermissionDescriptorParser
  for RuntimePermissionDescriptorParser
{
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
    if text.is_empty() {
      Err(AnyError::msg("Empty path not allowed"))
    } else {
      Ok(EnvDescriptor::new(text))
    }
  }

  fn parse_sys_descriptor(
    &self,
    text: &str,
  ) -> Result<deno_permissions::SysDescriptor, AnyError> {
    if text.is_empty() {
      Err(AnyError::msg("Empty sys not allowed"))
    } else {
      Ok(SysDescriptor(text.to_string()))
    }
  }

  fn parse_allow_run_descriptor(
    &self,
    text: &str,
  ) -> Result<AllowRunDescriptor, AnyError> {
    AllowRunDescriptor::parse(text, self.initial_cwd()?)
  }

  fn parse_deny_run_descriptor(
    &self,
    text: &str,
  ) -> Result<DenyRunDescriptor, AnyError> {
    Ok(DenyRunDescriptor::parse(text, self.initial_cwd()?))
  }

  fn parse_ffi_descriptor(
    &self,
    text: &str,
  ) -> Result<deno_permissions::FfiDescriptor, AnyError> {
    Ok(FfiDescriptor(self.resolve_path(text)?))
  }
}
