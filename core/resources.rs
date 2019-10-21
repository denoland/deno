// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated by
// the privileged side of Deno to refer to various rust objects that need to be
// referenced between multiple ops. For example, network sockets are resources.
// Resources may or may not correspond to a real operating system file
// descriptor (hence the different name).

use downcast_rs::Downcast;
use std;
use std::any::Any;
use std::collections::HashMap;
use std::io::Error;
use std::io::ErrorKind;

/// ResourceId is Deno's version of a file descriptor. ResourceId is also referred
/// to as rid in the code base.
pub type ResourceId = u32;

/// These store Deno's file descriptors. These are not necessarily the operating
/// system ones.
type ResourceMap = HashMap<ResourceId, Box<dyn Resource>>;

#[derive(Default)]
pub struct ResourceTable {
  map: ResourceMap,
  next_id: u32,
}

impl ResourceTable {
  pub fn get<T: Resource>(&self, rid: ResourceId) -> Result<&T, Error> {
    let resource = self.map.get(&rid).ok_or_else(bad_resource)?;
    let resource = &resource.downcast_ref::<T>().ok_or_else(bad_resource)?;
    Ok(resource)
  }

  pub fn get_mut<T: Resource>(
    &mut self,
    rid: ResourceId,
  ) -> Result<&mut T, Error> {
    let resource = self.map.get_mut(&rid).ok_or_else(bad_resource)?;
    let resource = resource.downcast_mut::<T>().ok_or_else(bad_resource)?;
    Ok(resource)
  }

  // TODO: resource id allocation should probably be randomized for security.
  fn next_rid(&mut self) -> ResourceId {
    let next_rid = self.next_id;
    self.next_id += 1;
    next_rid as ResourceId
  }

  pub fn add(&mut self, resource: Box<dyn Resource>) -> ResourceId {
    let rid = self.next_rid();
    let r = self.map.insert(rid, resource);
    assert!(r.is_none());
    rid
  }

  // close(2) is done by dropping the value. Therefore we just need to remove
  // the resource from the RESOURCE_TABLE.
  pub fn close(&mut self, rid: ResourceId) -> Result<(), Error> {
    let repr = self.map.remove(&rid).ok_or_else(bad_resource)?;
    // Give resource a chance to cleanup (notify tasks, etc.)
    repr.close();
    Ok(())
  }
}

/// Abstract type representing resource in Deno.
pub trait Resource: Downcast + Any + Send {
  /// Method that allows to cleanup resource.
  fn close(&self) {}

  fn inspect_repr(&self) -> &str {
    unimplemented!();
  }
}
impl_downcast!(Resource);

// TODO: probably bad error kind
pub fn bad_resource() -> Error {
  Error::new(ErrorKind::NotFound, "bad resource id")
}
