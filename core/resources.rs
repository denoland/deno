// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated
// by the privileged side of Deno to refer to various resources.  The simplest
// example are standard file system files and stdio - but there will be other
// resources added in the future that might not correspond to operating system
// level File Descriptors. To avoid confusion we call them "resources" not "file
// descriptors". This module implements a global resource table. Ops (AKA
// handlers) look up resources by their integer id here.

use crate::ErrBox;
use downcast_rs::Downcast;
use std;
use std::any::Any;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

/// Also referred to as rid.
pub type ResourceId = u32;

/// These store Deno's file descriptors. These are not necessarily the operating
/// system ones.
type ResourceMap = BTreeMap<ResourceId, Box<dyn Resource>>;

#[derive(Default)]
pub struct ResourceTable {
  map: ResourceMap,
  next_id: u32,
}

impl ResourceTable {
  pub fn get<T: Resource>(&self, rid: &ResourceId) -> Result<&T, ErrBox> {
    let resource = self.map.get(&rid).ok_or_else(bad_resource)?;
    let resource = &resource.downcast_ref::<T>().ok_or_else(bad_resource)?;
    Ok(resource)
  }

  pub fn get_mut<T: Resource>(
    &mut self,
    rid: &ResourceId,
  ) -> Result<&mut T, ErrBox> {
    let resource = self.map.get_mut(&rid).ok_or_else(bad_resource)?;
    let resource = resource.downcast_mut::<T>().ok_or_else(bad_resource)?;
    Ok(resource)
  }

  fn next_rid(&mut self) -> ResourceId {
    let next_rid = self.next_id;
    self.next_id += 1;
    next_rid as ResourceId
  }

  // TODO: change return type to ResourceId
  pub fn add(&mut self, resource: Box<dyn Resource>) -> ResourceId {
    let rid = self.next_rid();
    let r = self.map.insert(rid, resource);
    assert!(r.is_none());
    rid
  }

  // close(2) is done by dropping the value. Therefore we just need to remove
  // the resource from the RESOURCE_TABLE.
  pub fn close(&mut self, rid: &ResourceId) -> Result<(), ErrBox> {
    let repr = self.map.remove(rid).ok_or_else(bad_resource)?;
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

#[derive(Debug)]
struct StaticError(&'static str);

impl Error for StaticError {}

impl fmt::Display for StaticError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.pad(self.0)
  }
}

pub fn bad_resource() -> ErrBox {
  StaticError("bad resource id").into()
}
