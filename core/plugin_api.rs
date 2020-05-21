// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This file defines the public interface for dynamically loaded plugins.

// The plugin needs to do all interaction with the CLI crate through trait
// objects and function pointers. This ensures that no concrete internal methods
// (such as register_op and the closures created by it) can end up in the plugin
// shared library itself, which would cause segfaults when the plugin is
// unloaded and all functions in the plugin library are unmapped from memory.

pub use crate::Buf;
pub use crate::Op;
pub use crate::OpId;
pub use crate::Resource;
pub use crate::ResourceId;
pub use crate::ZeroCopyBuf;

pub type InitFn = fn(&mut dyn Interface);

pub type DispatchOpFn =
  fn(&mut dyn Interface, &[u8], Option<ZeroCopyBuf>) -> Op;

pub trait Interface {
  fn register_op(&mut self, name: &str, dispatcher: DispatchOpFn) -> OpId;
  fn resource_table(&mut self) -> &mut dyn ResourceTable;
}

/// Similar to `struct ResourceTable` for normal ops, but uses dynamic dispatch
/// instead of type parameters for its methods.
pub trait ResourceTable {
  fn add(&mut self, name: &str, resource: Box<dyn Resource>) -> ResourceId;
  fn close(&mut self, rid: ResourceId) -> Option<()>;
  fn get(&self, rid: ResourceId) -> Option<&dyn Resource>;
  fn get_mut(&mut self, rid: ResourceId) -> Option<&mut dyn Resource>;
  fn has(&self, rid: ResourceId) -> bool;
}
