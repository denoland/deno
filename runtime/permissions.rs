// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_path_util::normalize_path;
use deno_permissions::AllowRunDescriptor;
use deno_permissions::AllowRunDescriptorParseResult;
use deno_permissions::DenyRunDescriptor;
use deno_permissions::EnvDescriptor;
use deno_permissions::FfiDescriptor;
use deno_permissions::ImportDescriptor;
use deno_permissions::NetDescriptor;
use deno_permissions::PathQueryDescriptor;
use deno_permissions::ReadDescriptor;
use deno_permissions::RunQueryDescriptor;
use deno_permissions::SysDescriptor;
use deno_permissions::WriteDescriptor;

#[derive(Debug)]
pub struct RuntimePermissionDescriptorParser {
  fs: deno_fs::FileSystemRc,
}

impl RuntimePermissionDescriptorParser {
  pub fn new(fs: deno_fs::FileSystemRc) -> Self {
    Self { fs }
  }

  fn resolve_from_cwd(&self, path: &str) -> Result<PathBuf, AnyError> {
    if path.is_empty() {
      bail!("Empty path is not allowed");
    }
    let path = Path::new(path);
    if path.is_absolute() {
      Ok(normalize_path(path))
    } else {
      let cwd = self.resolve_cwd()?;
      Ok(normalize_path(cwd.join(path)))
    }
  }

  fn resolve_cwd(&self) -> Result<PathBuf, AnyError> {
    self
      .fs
      .cwd()
      .map_err(|e| e.into_io_error())
      .context("failed resolving cwd")
  }
}

impl deno_permissions::PermissionDescriptorParser
  for RuntimePermissionDescriptorParser
{
  fn parse_read_descriptor(
    &self,
    text: &str,
  ) -> Result<ReadDescriptor, AnyError> {
    Ok(ReadDescriptor(self.resolve_from_cwd(text)?))
  }

  fn parse_write_descriptor(
    &self,
    text: &str,
  ) -> Result<WriteDescriptor, AnyError> {
    Ok(WriteDescriptor(self.resolve_from_cwd(text)?))
  }

  fn parse_net_descriptor(
    &self,
    text: &str,
  ) -> Result<NetDescriptor, AnyError> {
    NetDescriptor::parse(text)
  }

  fn parse_import_descriptor(
    &self,
    text: &str,
  ) -> Result<ImportDescriptor, AnyError> {
    ImportDescriptor::parse(text)
  }

  fn parse_env_descriptor(
    &self,
    text: &str,
  ) -> Result<EnvDescriptor, AnyError> {
    if text.is_empty() {
      Err(AnyError::msg("Empty env not allowed"))
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
      Ok(SysDescriptor::parse(text.to_string())?)
    }
  }

  fn parse_allow_run_descriptor(
    &self,
    text: &str,
  ) -> Result<AllowRunDescriptorParseResult, AnyError> {
    Ok(AllowRunDescriptor::parse(text, &self.resolve_cwd()?)?)
  }

  fn parse_deny_run_descriptor(
    &self,
    text: &str,
  ) -> Result<DenyRunDescriptor, AnyError> {
    Ok(DenyRunDescriptor::parse(text, &self.resolve_cwd()?))
  }

  fn parse_ffi_descriptor(
    &self,
    text: &str,
  ) -> Result<deno_permissions::FfiDescriptor, AnyError> {
    Ok(FfiDescriptor(self.resolve_from_cwd(text)?))
  }

  // queries

  fn parse_path_query(
    &self,
    path: &str,
  ) -> Result<PathQueryDescriptor, AnyError> {
    Ok(PathQueryDescriptor {
      resolved: self.resolve_from_cwd(path)?,
      requested: path.to_string(),
    })
  }

  fn parse_run_query(
    &self,
    requested: &str,
  ) -> Result<RunQueryDescriptor, AnyError> {
    if requested.is_empty() {
      bail!("Empty run query is not allowed");
    }
    RunQueryDescriptor::parse(requested)
  }
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use deno_fs::RealFs;
  use deno_permissions::PermissionDescriptorParser;

  use super::*;

  #[test]
  fn test_handle_empty_value() {
    let parser = RuntimePermissionDescriptorParser::new(Arc::new(RealFs));
    assert!(parser.parse_read_descriptor("").is_err());
    assert!(parser.parse_write_descriptor("").is_err());
    assert!(parser.parse_env_descriptor("").is_err());
    assert!(parser.parse_net_descriptor("").is_err());
    assert!(parser.parse_ffi_descriptor("").is_err());
    assert!(parser.parse_path_query("").is_err());
    assert!(parser.parse_run_query("").is_err());
  }
}
