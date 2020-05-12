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
pub use crate::ResourceTable;
pub use crate::ZeroCopyBuf;

pub type InitFn = fn(&mut dyn Interface);

pub type DispatchOpFn =
  fn(&mut dyn Interface, &[u8], Option<ZeroCopyBuf>) -> Op;

/// Equivilent to ResourceTable for normal ops, but only includes
/// non generic("boxed") versions of get, get_mut, and remove.
/// We can't have generic functions on a trait and use it as a object
/// this is a limtation of rust.
pub trait WrappedResourceTable {
  fn has(&self, rid: ResourceId) -> bool;
  fn get_boxed(&self, rid: ResourceId) -> Option<&dyn Resource>;
  fn get_mut_boxed(&mut self, rid: ResourceId) -> Option<&mut dyn Resource>;
  fn add(&mut self, name: &str, resource: Box<dyn Resource>) -> ResourceId;
  fn entries(&self) -> Vec<(ResourceId, String)>;
  fn close(&mut self, rid: ResourceId) -> Option<()>;
  fn remove_boxed(&mut self, rid: ResourceId) -> Option<Box<dyn Resource>>;
}

pub trait Interface {
  fn register_op(&mut self, name: &str, dispatcher: DispatchOpFn) -> OpId;
  fn resource_table<'a>(&'a mut self) -> Box<dyn WrappedResourceTable + 'a>;
}
