// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This file defines the public interface for dynamically loaded plugins.

// The plugin needs to do all interaction with the CLI crate through trait
// objects and function pointers. This ensures that no concrete internal methods
// (such as register_op and the closures created by it) can end up in the plugin
// shared library itself, which would cause segfaults when the plugin is
// unloaded and all functions in the plugin library are unmapped from memory.

pub use crate::Op;
pub use crate::OpId;
pub use crate::ZeroCopyBuf;

pub type InitFn = fn(&mut dyn Interface);

pub type DispatchOpFn = dyn Fn(&mut dyn Interface, &mut [ZeroCopyBuf]) -> Op;

pub trait Interface {
  fn register_op(&mut self, name: &str, dispatcher: Box<DispatchOpFn>) -> OpId;
}
