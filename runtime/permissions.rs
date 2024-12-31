// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_path_util::normalize_path;
use deno_permissions::AllowRunDescriptor;
use deno_permissions::AllowRunDescriptorParseResult;
use deno_permissions::DenyRunDescriptor;
use deno_permissions::EnvDescriptor;
use deno_permissions::FfiDescriptor;
use deno_permissions::ImportDescriptor;
use deno_permissions::NetDescriptor;
use deno_permissions::PathQueryDescriptor;
use deno_permissions::PathResolveError;
use deno_permissions::ReadDescriptor;
use deno_permissions::RunDescriptorParseError;
use deno_permissions::RunQueryDescriptor;
use deno_permissions::SysDescriptor;
use deno_permissions::SysDescriptorParseError;
use deno_permissions::WriteDescriptor;

#[derive(Debug)]
pub struct RuntimePermissionDescriptorParser<
  TSys: sys_traits::EnvCurrentDir + Send + Sync,
> {
  sys: TSys,
}

impl<TSys: sys_traits::EnvCurrentDir + Send + Sync>
  RuntimePermissionDescriptorParser<TSys>
{
  pub fn new(sys: TSys) -> Self {
    Self { sys }
  }

  fn resolve_from_cwd(&self, path: &str) -> Result<PathBuf, PathResolveError> {
    if path.is_empty() {
      return Err(PathResolveError::EmptyPath);
    }
    let path = Path::new(path);
    if path.is_absolute() {
      Ok(normalize_path(path))
    } else {
      let cwd = self.resolve_cwd()?;
      Ok(normalize_path(cwd.join(path)))
    }
  }

  fn resolve_cwd(&self) -> Result<PathBuf, PathResolveError> {
    self
      .sys
      .env_current_dir()
      .map_err(PathResolveError::CwdResolve)
  }
}

impl<TSys: sys_traits::EnvCurrentDir + Send + Sync + std::fmt::Debug>
  deno_permissions::PermissionDescriptorParser
  for RuntimePermissionDescriptorParser<TSys>
{
  fn parse_read_descriptor(
    &self,
    text: &str,
  ) -> Result<ReadDescriptor, PathResolveError> {
    Ok(ReadDescriptor(self.resolve_from_cwd(text)?))
  }

  fn parse_write_descriptor(
    &self,
    text: &str,
  ) -> Result<WriteDescriptor, PathResolveError> {
    Ok(WriteDescriptor(self.resolve_from_cwd(text)?))
  }

  fn parse_net_descriptor(
    &self,
    text: &str,
  ) -> Result<NetDescriptor, deno_permissions::NetDescriptorParseError> {
    NetDescriptor::parse(text)
  }

  fn parse_import_descriptor(
    &self,
    text: &str,
  ) -> Result<ImportDescriptor, deno_permissions::NetDescriptorParseError> {
    ImportDescriptor::parse(text)
  }

  fn parse_env_descriptor(
    &self,
    text: &str,
  ) -> Result<EnvDescriptor, deno_permissions::EnvDescriptorParseError> {
    if text.is_empty() {
      Err(deno_permissions::EnvDescriptorParseError)
    } else {
      Ok(EnvDescriptor::new(text))
    }
  }

  fn parse_sys_descriptor(
    &self,
    text: &str,
  ) -> Result<SysDescriptor, SysDescriptorParseError> {
    if text.is_empty() {
      Err(SysDescriptorParseError::Empty)
    } else {
      Ok(SysDescriptor::parse(text.to_string())?)
    }
  }

  fn parse_allow_run_descriptor(
    &self,
    text: &str,
  ) -> Result<AllowRunDescriptorParseResult, RunDescriptorParseError> {
    Ok(AllowRunDescriptor::parse(text, &self.resolve_cwd()?)?)
  }

  fn parse_deny_run_descriptor(
    &self,
    text: &str,
  ) -> Result<DenyRunDescriptor, PathResolveError> {
    Ok(DenyRunDescriptor::parse(text, &self.resolve_cwd()?))
  }

  fn parse_ffi_descriptor(
    &self,
    text: &str,
  ) -> Result<FfiDescriptor, PathResolveError> {
    Ok(FfiDescriptor(self.resolve_from_cwd(text)?))
  }

  // queries

  fn parse_path_query(
    &self,
    path: &str,
  ) -> Result<PathQueryDescriptor, PathResolveError> {
    Ok(PathQueryDescriptor {
      resolved: self.resolve_from_cwd(path)?,
      requested: path.to_string(),
    })
  }

  fn parse_run_query(
    &self,
    requested: &str,
  ) -> Result<RunQueryDescriptor, RunDescriptorParseError> {
    if requested.is_empty() {
      return Err(RunDescriptorParseError::EmptyRunQuery);
    }
    RunQueryDescriptor::parse(requested)
      .map_err(RunDescriptorParseError::PathResolve)
  }
}

#[cfg(test)]
mod test {
  use deno_permissions::PermissionDescriptorParser;

  use super::*;

  #[test]
  fn test_handle_empty_value() {
    let parser =
      RuntimePermissionDescriptorParser::new(sys_traits::impls::RealSys);
    assert!(parser.parse_read_descriptor("").is_err());
    assert!(parser.parse_write_descriptor("").is_err());
    assert!(parser.parse_env_descriptor("").is_err());
    assert!(parser.parse_net_descriptor("").is_err());
    assert!(parser.parse_ffi_descriptor("").is_err());
    assert!(parser.parse_path_query("").is_err());
    assert!(parser.parse_run_query("").is_err());
  }
}
