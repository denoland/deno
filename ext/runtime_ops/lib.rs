// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

pub mod fs_events;
pub mod http;
pub mod os;
pub mod permissions;
pub mod process;
pub mod runtime;
pub mod signal;
pub mod tty;
mod utils;
pub mod web_worker;
pub mod worker_host;

use serde::Deserialize;
use std::fmt;

/// Quadri-state value for storing permission state
#[derive(
  Eq, PartialEq, Default, Debug, Clone, Copy, Deserialize, PartialOrd,
)]
pub enum PermissionState {
  Granted = 0,
  GrantedPartial = 1,
  #[default]
  Prompt = 2,
  Denied = 3,
}

impl fmt::Display for PermissionState {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      PermissionState::Granted => f.pad("granted"),
      PermissionState::GrantedPartial => f.pad("granted-partial"),
      PermissionState::Prompt => f.pad("prompt"),
      PermissionState::Denied => f.pad("denied"),
    }
  }
}

use deno_core::error::AnyError;
use deno_core::OpState;
use std::path::Path;

/// Helper for checking unstable features. Used for sync ops.
pub fn check_unstable(state: &OpState, feature: &str, api_name: &str) {
  // TODO(bartlomieju): replace with `state.feature_checker.check_or_exit`
  // once we phase out `check_or_exit_with_legacy_fallback`
  state
    .feature_checker
    .check_or_exit_with_legacy_fallback(feature, api_name);
}

pub struct TestingFeaturesEnabled(pub bool);

pub trait RuntimePermissions {
  fn check_read(
    &mut self,
    _path: &Path,
    _api_name: &str,
  ) -> Result<(), AnyError> {
    unimplemented!()
  }
  fn check_read_blind(
    &mut self,
    _p: &Path,
    _display: &str,
    _api_name: &str,
  ) -> Result<(), AnyError> {
    unimplemented!()
  }
  fn check_env(&mut self, _var: &str) -> Result<(), AnyError> {
    unimplemented!()
  }
  fn check_env_all(&mut self) -> Result<(), AnyError> {
    unimplemented!()
  }
  fn check_sys(
    &mut self,
    _kind: &str,
    _api_name: &str,
  ) -> Result<(), AnyError> {
    unimplemented!()
  }
  fn check_run(&mut self, _cmd: &str, _api_name: &str) -> Result<(), AnyError> {
    unimplemented!()
  }
  fn check_run_all(&mut self, _api_name: &str) -> Result<(), AnyError> {
    unimplemented!()
  }

  // Queries
  fn query_read(&self, _path: Option<&Path>) -> PermissionState {
    unimplemented!()
  }
  fn query_write(&self, _path: Option<&Path>) -> PermissionState {
    unimplemented!()
  }
  fn query_net<T: AsRef<str>>(
    &self,
    _host: Option<&(T, Option<u16>)>,
  ) -> PermissionState {
    unimplemented!()
  }
  fn query_env(&self, _var: Option<&str>) -> PermissionState {
    unimplemented!()
  }
  fn query_sys(&self, _kind: Option<&str>) -> PermissionState {
    unimplemented!()
  }
  fn query_run(&self, _cmd: Option<&str>) -> PermissionState {
    unimplemented!()
  }
  fn query_ffi(&self, _path: Option<&Path>) -> PermissionState {
    unimplemented!()
  }
  fn query_hrtime(&self) -> PermissionState {
    unimplemented!()
  }

  fn revoke_read(&mut self, _path: Option<&Path>) -> PermissionState {
    unimplemented!()
  }
  fn revoke_write(&mut self, _path: Option<&Path>) -> PermissionState {
    unimplemented!()
  }
  fn revoke_net<T: AsRef<str>>(
    &self,
    _host: Option<&(T, Option<u16>)>,
  ) -> PermissionState {
    unimplemented!()
  }
  fn revoke_env(&mut self, _var: Option<&str>) -> PermissionState {
    unimplemented!()
  }
  fn revoke_sys(&mut self, _kind: Option<&str>) -> PermissionState {
    unimplemented!()
  }
  fn revoke_run(&mut self, _cmd: Option<&str>) -> PermissionState {
    unimplemented!()
  }
  fn revoke_ffi(&mut self, _path: Option<&Path>) -> PermissionState {
    unimplemented!()
  }
  fn revoke_hrtime(&mut self) -> PermissionState {
    unimplemented!()
  }

  fn request_read(&mut self, _path: Option<&Path>) -> PermissionState {
    unimplemented!()
  }
  fn request_write(&mut self, _path: Option<&Path>) -> PermissionState {
    unimplemented!()
  }
  fn request_net<T: AsRef<str>>(
    &self,
    _host: Option<&(T, Option<u16>)>,
  ) -> PermissionState {
    unimplemented!()
  }
  fn request_env(&mut self, _var: Option<&str>) -> PermissionState {
    unimplemented!()
  }
  fn request_sys(&mut self, _kind: Option<&str>) -> PermissionState {
    unimplemented!()
  }
  fn request_run(&mut self, _cmd: Option<&str>) -> PermissionState {
    unimplemented!()
  }
  fn request_ffi(&mut self, _path: Option<&Path>) -> PermissionState {
    unimplemented!()
  }
  fn request_hrtime(&mut self) -> PermissionState {
    unimplemented!()
  }
}
