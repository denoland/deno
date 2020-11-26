// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated by
// the privileged side of Deno to refer to various rust objects that need to be
// referenced between multiple ops. For example, network sockets are resources.
// Resources may or may not correspond to a real operating system file
// descriptor (hence the different name).

use crate::resources2::ResourceId;
use std::any::Any;
use std::collections::HashMap;

/// These store Deno's file descriptors. These are not necessarily the operating
/// system ones.
type ResourceMap = HashMap<ResourceId, (String, Box<dyn Any>)>;

/// Map-like data structure storing Deno's resources (equivalent to file descriptors).
///
/// Provides basic methods for element access. A resource can be of any type.
/// Different types of resources can be stored in the same map, and provided
/// with a name for description.
///
/// Each resource is identified through a _resource ID (rid)_, which acts as
/// the key in the map.
#[derive(Default)]
pub struct ResourceTable {
  map: ResourceMap,
  next_id: u32,
}

impl ResourceTable {
  /// Checks if the given resource ID is contained.
  pub fn has(&self, rid: ResourceId) -> bool {
    self.map.contains_key(&rid)
  }

  /// Returns a shared reference to a resource.
  ///
  /// Returns `None`, if `rid` is not stored or has a type different from `T`.
  pub fn get<T: Any>(&self, rid: ResourceId) -> Option<&T> {
    let (_, resource) = self.map.get(&rid)?;
    resource.downcast_ref::<T>()
  }

  /// Returns a mutable reference to a resource.
  ///
  /// Returns `None`, if `rid` is not stored or has a type different from `T`.
  pub fn get_mut<T: Any>(&mut self, rid: ResourceId) -> Option<&mut T> {
    let (_, resource) = self.map.get_mut(&rid)?;
    resource.downcast_mut::<T>()
  }

  // TODO: resource id allocation should probably be randomized for security.
  fn next_rid(&mut self) -> ResourceId {
    let next_rid = self.next_id;
    self.next_id += 1;
    next_rid as ResourceId
  }

  /// Inserts a resource, taking ownership of it.
  ///
  /// The resource type is erased at runtime and must be statically known
  /// when retrieving it through `get()`.
  ///
  /// Returns a unique resource ID, which acts as a key for this resource.
  pub fn add(&mut self, name: &str, resource: Box<dyn Any>) -> ResourceId {
    let rid = self.next_rid();
    let r = self.map.insert(rid, (name.to_string(), resource));
    assert!(r.is_none());
    rid
  }

  /// Returns a map of resource IDs to names.
  ///
  /// The name is the one specified during `add()`. To access resources themselves,
  /// use the `get()` or `get_mut()` functions.
  pub fn entries(&self) -> HashMap<ResourceId, String> {
    self
      .map
      .iter()
      .map(|(key, (name, _resource))| (*key, name.clone()))
      .collect()
  }

  // close(2) is done by dropping the value. Therefore we just need to remove
  // the resource from the resource table.
  pub fn close(&mut self, rid: ResourceId) -> Option<()> {
    self.map.remove(&rid).map(|(_name, _resource)| ())
  }

  /// Removes the resource identified by `rid` and returns it.
  ///
  /// When the provided `rid` is stored, the associated resource will be removed.
  /// Otherwise, nothing happens and `None` is returned.
  ///
  /// If the type `T` matches the resource's type, the resource will be returned.
  /// If the type mismatches, `None` is returned, but the resource is still removed.
  pub fn remove<T: Any>(&mut self, rid: ResourceId) -> Option<Box<T>> {
    if let Some((_name, resource)) = self.map.remove(&rid) {
      let res = match resource.downcast::<T>() {
        Ok(res) => Some(res),
        Err(_e) => None,
      };
      return res;
    }
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct FakeResource {
    not_empty: u128,
  }

  impl FakeResource {
    fn new(value: u128) -> FakeResource {
      FakeResource { not_empty: value }
    }
  }

  #[test]
  fn test_create_resource_table_default() {
    let table = ResourceTable::default();
    assert_eq!(table.map.len(), 0);
  }

  #[test]
  fn test_add_to_resource_table_not_empty() {
    let mut table = ResourceTable::default();
    table.add("fake1", Box::new(FakeResource::new(1)));
    table.add("fake2", Box::new(FakeResource::new(2)));
    assert_eq!(table.map.len(), 2);
  }

  #[test]
  fn test_add_to_resource_table_are_contiguous() {
    let mut table = ResourceTable::default();
    let rid1 = table.add("fake1", Box::new(FakeResource::new(1)));
    let rid2 = table.add("fake2", Box::new(FakeResource::new(2)));
    assert_eq!(rid1 + 1, rid2);
  }

  #[test]
  fn test_get_from_resource_table_is_what_was_given() {
    let mut table = ResourceTable::default();
    let rid = table.add("fake", Box::new(FakeResource::new(7)));
    let resource = table.get::<FakeResource>(rid);
    assert_eq!(resource.unwrap().not_empty, 7);
  }

  #[test]
  fn test_remove_from_resource_table() {
    let mut table = ResourceTable::default();
    let rid1 = table.add("fake1", Box::new(FakeResource::new(1)));
    let rid2 = table.add("fake2", Box::new(FakeResource::new(2)));
    assert_eq!(table.map.len(), 2);
    table.close(rid1);
    assert_eq!(table.map.len(), 1);
    table.close(rid2);
    assert_eq!(table.map.len(), 0);
  }

  #[test]
  fn test_take_from_resource_table() {
    let mut table = ResourceTable::default();
    let rid1 = table.add("fake1", Box::new(FakeResource::new(1)));
    let rid2 = table.add("fake2", Box::new(FakeResource::new(2)));
    assert_eq!(table.map.len(), 2);
    let res1 = table.remove::<FakeResource>(rid1);
    assert_eq!(table.map.len(), 1);
    assert!(res1.is_some());
    let res2 = table.remove::<FakeResource>(rid2);
    assert_eq!(table.map.len(), 0);
    assert!(res2.is_some());
  }
}
