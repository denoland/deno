// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use crate::AllowRunDescriptor;
use crate::AllowRunDescriptorParseResult;
use crate::DenyRunDescriptor;
use crate::EnvDescriptor;
use crate::FfiDescriptor;
use crate::ImportDescriptor;
use crate::NetDescriptor;
use crate::PathDescriptor;
use crate::PathQueryDescriptor;
use crate::PathResolveError;
use crate::ReadDescriptor;
use crate::RunDescriptorParseError;
use crate::RunQueryDescriptor;
use crate::SpecialFilePathQueryDescriptor;
use crate::SysDescriptor;
use crate::SysDescriptorParseError;
use crate::WriteDescriptor;

#[sys_traits::auto_impl]
pub trait RuntimePermissionDescriptorParserSys:
  crate::which::WhichSys + sys_traits::FsCanonicalize + Send + Sync
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

  fn parse_path_descriptor(
    &self,
    path: Cow<'_, Path>,
  ) -> Result<PathDescriptor, PathResolveError> {
    PathDescriptor::new(&self.sys, path)
  }
}

impl<TSys: RuntimePermissionDescriptorParserSys + std::fmt::Debug>
  crate::PermissionDescriptorParser
  for RuntimePermissionDescriptorParser<TSys>
{
  fn parse_read_descriptor(
    &self,
    text: &str,
  ) -> Result<ReadDescriptor, PathResolveError> {
    Ok(ReadDescriptor(
      self.parse_path_descriptor(Cow::Borrowed(Path::new(text)))?,
    ))
  }

  fn parse_write_descriptor(
    &self,
    text: &str,
  ) -> Result<WriteDescriptor, PathResolveError> {
    Ok(WriteDescriptor(
      self.parse_path_descriptor(Cow::Borrowed(Path::new(text)))?,
    ))
  }

  fn parse_net_descriptor(
    &self,
    text: &str,
  ) -> Result<NetDescriptor, crate::NetDescriptorParseError> {
    NetDescriptor::parse_for_list(text)
  }

  fn parse_import_descriptor(
    &self,
    text: &str,
  ) -> Result<ImportDescriptor, crate::NetDescriptorParseError> {
    ImportDescriptor::parse_for_list(text)
  }

  fn parse_env_descriptor(
    &self,
    text: &str,
  ) -> Result<EnvDescriptor, crate::EnvDescriptorParseError> {
    if text.is_empty() {
      Err(crate::EnvDescriptorParseError)
    } else {
      Ok(EnvDescriptor::new(Cow::Borrowed(text)))
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
    Ok(FfiDescriptor(
      self.parse_path_descriptor(Cow::Borrowed(Path::new(text)))?,
    ))
  }

  // queries

  fn parse_path_query<'a>(
    &self,
    path: Cow<'a, Path>,
  ) -> Result<PathQueryDescriptor<'a>, PathResolveError> {
    PathQueryDescriptor::new(&self.sys, path)
  }

  fn parse_special_file_descriptor<'a>(
    &self,
    path: PathQueryDescriptor<'a>,
  ) -> Result<SpecialFilePathQueryDescriptor<'a>, PathResolveError> {
    SpecialFilePathQueryDescriptor::parse(&self.sys, path)
  }

  fn parse_net_query(
    &self,
    text: &str,
  ) -> Result<NetDescriptor, crate::NetDescriptorParseError> {
    NetDescriptor::parse_for_query(text)
  }

  fn parse_run_query<'a>(
    &self,
    requested: &'a str,
  ) -> Result<RunQueryDescriptor<'a>, RunDescriptorParseError> {
    if requested.is_empty() {
      return Err(RunDescriptorParseError::EmptyRunQuery);
    }
    RunQueryDescriptor::parse(requested, &self.sys)
      .map_err(RunDescriptorParseError::PathResolve)
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::PermissionDescriptorParser;

  #[test]
  fn test_handle_empty_value() {
    let parser =
      RuntimePermissionDescriptorParser::new(sys_traits::impls::RealSys);
    assert!(parser.parse_read_descriptor("").is_err());
    assert!(parser.parse_write_descriptor("").is_err());
    assert!(parser.parse_env_descriptor("").is_err());
    assert!(parser.parse_net_descriptor("").is_err());
    assert!(parser.parse_ffi_descriptor("").is_err());
    assert!(
      parser
        .parse_path_query(Cow::Borrowed(Path::new("")))
        .is_err()
    );
    assert!(parser.parse_net_query("").is_err());
    assert!(parser.parse_run_query("").is_err());
  }
}
