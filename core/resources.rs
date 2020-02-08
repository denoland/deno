// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated by
// the privileged side of Deno to refer to various rust objects that need to be
// referenced between multiple ops. For example, network sockets are resources.
// Resources may or may not correspond to a real operating system file
// descriptor (hence the different name).

use downcast_rs::Downcast;
use std;
use std::any::Any;
use std::collections::HashMap;

/// ResourceId is Deno's version of a file descriptor. ResourceId is also referred
/// to as rid in the code base.
pub type ResourceId = u32;

/// These store Deno's file descriptors. These are not necessarily the operating
/// system ones.
type ResourceMap = HashMap<ResourceId, (String, Box<dyn Resource>)>;

#[derive(Default)]
pub struct ResourceTable {
  map: ResourceMap,
  next_id: u32,
}

impl ResourceTable {
  pub fn get<T: Resource>(&self, rid: ResourceId) -> Option<&T> {
    if let Some((_name, resource)) = self.map.get(&rid) {
      return resource.downcast_ref::<T>();
    }

    None
  }

  pub fn get_mut<T: Resource>(&mut self, rid: ResourceId) -> Option<&mut T> {
    if let Some((_name, resource)) = self.map.get_mut(&rid) {
      return resource.downcast_mut::<T>();
    }

    None
  }

  // TODO: resource id allocation should probably be randomized for security.
  fn next_rid(&mut self) -> ResourceId {
    let next_rid = self.next_id;
    self.next_id += 1;
    next_rid as ResourceId
  }

  pub fn add(&mut self, name: &str, resource: Box<dyn Resource>) -> ResourceId {
    let rid = self.next_rid();
    let r = self.map.insert(rid, (name.to_string(), resource));
    assert!(r.is_none());
    rid
  }

  pub fn entries(&self) -> Vec<(ResourceId, String)> {
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
}

/// Abstract type representing resource in Deno.
///
/// The only thing it does is implementing `Downcast` trait
/// that allows to cast resource to concrete type in `TableResource::get`
/// and `TableResource::get_mut` methods.
pub trait Resource: Downcast + Any + Send {}
impl<T> Resource for T where T: Downcast + Any + Send {}
impl_downcast!(Resource);
