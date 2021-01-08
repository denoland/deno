// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// This file defines the public interface for dynamically loaded plugins.

// The plugin needs to do all interaction with the CLI crate through trait
// objects and function pointers. This ensures that no concrete internal methods
// (such as register_op and the closures created by it) can end up in the plugin
// shared library itself, which would cause segfaults when the plugin is
// unloaded and all functions in the plugin library are unmapped from memory.

pub use crate::Op;
pub use crate::OpId;
pub use crate::ZeroCopyBuf;

use crate::error::AnyError;
use futures::Future;
use serde_json::Value;

pub type InitFn = fn(&mut dyn Interface);

pub type DispatchOpFn = dyn Fn(&mut dyn Interface, &mut [ZeroCopyBuf]) -> Op;
pub type JsonOpSync = dyn Fn(
  &mut dyn Interface,
  Value,
  &mut [ZeroCopyBuf],
) -> Result<Value, AnyError>;
pub type JsonOpAsync = dyn Fn(
  &mut dyn Interface,
  Value,
  &mut [ZeroCopyBuf],
) -> (dyn Future<Output = Result<Value, AnyError>>);

pub trait Interface {
  fn register_op(&mut self, name: &str, dispatcher: Box<DispatchOpFn>) -> OpId;
  fn register_json_op_sync(
    &mut self,
    name: &str,
    dispatcher: Box<JsonOpSync>,
  ) -> OpId;
  fn register_json_op_async(
    &mut self,
    name: &str,
    dispatcher: Box<JsonOpAsync>,
  ) -> OpId;
}
