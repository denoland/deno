// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

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
use deno_permissions::SpecialFilePathDescriptor;
use deno_permissions::SysDescriptor;
use deno_permissions::SysDescriptorParseError;
use deno_permissions::WriteDescriptor;

#[sys_traits::auto_impl]
pub trait RuntimePermissionDescriptorParserSys:
  deno_permissions::which::WhichSys + sys_traits::FsCanonicalize + Send + Sync
{
}

#[derive(Debug)]
pub struct RuntimePermissionDescriptorParser<
  TSys: RuntimePermissionDescriptorParserSys,
> {
  sys: TSys,
}

impl<TSys: RuntimePermissionDescriptorParserSys>
  RuntimePermissionDescriptorParser<TSys>
{
  pub fn new(sys: TSys) -> Self {
    Self { sys }
  }

  fn resolve_cwd(&self) -> Result<PathBuf, PathResolveError> {
    self
      .sys
      .env_current_dir()
      .map_err(PathResolveError::CwdResolve)
  }
}

impl<TSys: RuntimePermissionDescriptorParserSys + std::fmt::Debug>
  deno_permissions::PermissionDescriptorParser
  for RuntimePermissionDescriptorParser<TSys>
{
  fn parse_read_descriptor(
    &self,
    text: &str,
  ) -> Result<ReadDescriptor, PathResolveError> {
    Ok(ReadDescriptor(self.parse_path_query(text)?))
  }

  fn parse_write_descriptor(
    &self,
    text: &str,
  ) -> Result<WriteDescriptor, PathResolveError> {
    Ok(WriteDescriptor(self.parse_path_query(text)?))
  }

  fn parse_net_descriptor(
    &self,
    text: &str,
  ) -> Result<NetDescriptor, deno_permissions::NetDescriptorParseError> {
    NetDescriptor::parse_for_list(text)
  }

  fn parse_import_descriptor(
    &self,
    text: &str,
  ) -> Result<ImportDescriptor, deno_permissions::NetDescriptorParseError> {
    ImportDescriptor::parse_for_list(text)
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
    Ok(AllowRunDescriptor::parse(
      text,
      &self.resolve_cwd()?,
      &self.sys,
    )?)
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
    Ok(FfiDescriptor(self.parse_path_query(text)?))
  }

  // queries

  fn parse_path_query_from_path(
    &self,
    path: Cow<'_, Path>,
  ) -> Result<PathQueryDescriptor, PathResolveError> {
    PathQueryDescriptor::new(&self.sys, path)
  }

  fn parse_special_file_descriptor(
    &self,
    path: PathQueryDescriptor,
  ) -> Result<SpecialFilePathDescriptor, PathResolveError> {
    SpecialFilePathDescriptor::parse(&self.sys, path)
  }

  fn parse_net_query(
    &self,
    text: &str,
  ) -> Result<NetDescriptor, deno_permissions::NetDescriptorParseError> {
    NetDescriptor::parse_for_query(text)
  }

  fn parse_run_query(
    &self,
    requested: &str,
  ) -> Result<RunQueryDescriptor, RunDescriptorParseError> {
    if requested.is_empty() {
      return Err(RunDescriptorParseError::EmptyRunQuery);
    }
    RunQueryDescriptor::parse(requested, &self.sys)
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
    assert!(parser.parse_net_query("").is_err());
    assert!(parser.parse_run_query("").is_err());
  }
}
