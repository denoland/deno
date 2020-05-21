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

use std::cell::Ref;
use std::cell::RefMut;

pub type InitFn = fn(&mut dyn Interface);

pub type DispatchOpFn =
  fn(&mut dyn Interface, &[u8], Option<ZeroCopyBuf>) -> Op;

/// Equivalent to ResourceTable for normal ops, but uses dynamic dispatch
/// rather than type parameters for the `get`, `get_mut`, and `remove` methods.
pub trait ResourceTable {
  fn add(&mut self, name: &str, resource: Box<dyn Resource>) -> ResourceId;
  fn get(&self, rid: ResourceId) -> Option<Ref<dyn Resource>>;
  fn get_mut(&mut self, rid: ResourceId) -> Option<RefMut<dyn Resource>>;
  fn remove(&mut self, rid: ResourceId) -> Option<Box<dyn Resource>>;

  // Convenience functions -- these can be automatically implemented using the
  // trait methods above.
  fn has(&self, rid: ResourceId) -> bool {
    self.get(rid).map(|_| true).unwrap_or(false)
  }
  fn close(&mut self, rid: ResourceId) -> Option<()> {
    self.remove(rid).map(|_| ())
  }
}

pub trait Interface {
  fn register_op(&mut self, name: &str, dispatcher: DispatchOpFn) -> OpId;
  fn resource_table(&mut self) -> &mut dyn ResourceTable;
}
