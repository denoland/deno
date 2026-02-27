// Copyright 2018-2025 the Deno authors. MIT license.

use super::Resource;
use super::ResourceHandle;
use super::ResourceHandleFd;
use super::ResourceHandleSocket;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A `ResourceId` is an integer value referencing a resource. It could be
/// considered to be the Deno equivalent of a `file descriptor` in POSIX like
/// operating systems. Elsewhere in the code base it is commonly abbreviated
/// to `rid`.
// TODO: use `u64` instead?
pub type ResourceId = u32;
/// Map-like data structure storing Deno's resources (equivalent to file
/// descriptors).
///
/// Provides basic methods for element access. A resource can be of any type.
/// Different types of resources can be stored in the same map, and provided
/// with a name for description.
///
/// Each resource is identified through a _resource ID (rid)_, which acts as
/// the key in the map.
#[derive(Default)]
pub struct ResourceTable {
  index: BTreeMap<ResourceId, Rc<dyn Resource>>,
  next_rid: ResourceId,
}

impl ResourceTable {
  /// Returns the number of resources currently active in the resource table.
  /// Resources taken from the table do not contribute to this count.
  pub fn len(&self) -> usize {
    self.index.len()
  }

  /// Returns whether this table is empty.
  pub fn is_empty(&self) -> bool {
    self.index.is_empty()
  }

  /// Inserts resource into the resource table, which takes ownership of it.
  ///
  /// The resource type is erased at runtime and must be statically known
  /// when retrieving it through `get()`.
  ///
  /// Returns a unique resource ID, which acts as a key for this resource.
  pub fn add<T: Resource>(&mut self, resource: T) -> ResourceId {
    self.add_rc(Rc::new(resource))
  }

  /// Inserts a `Rc`-wrapped resource into the resource table.
  ///
  /// The resource type is erased at runtime and must be statically known
  /// when retrieving it through `get()`.
  ///
  /// Returns a unique resource ID, which acts as a key for this resource.
  pub fn add_rc<T: Resource>(&mut self, resource: Rc<T>) -> ResourceId {
    let resource = resource as Rc<dyn Resource>;
    self.add_rc_dyn(resource)
  }

  pub fn add_rc_dyn(&mut self, resource: Rc<dyn Resource>) -> ResourceId {
    let rid = self.next_rid;
    let removed_resource = self.index.insert(rid, resource);
    assert!(removed_resource.is_none());
    self.next_rid += 1;
    rid
  }

  /// Returns true if any resource with the given `rid` exists.
  pub fn has(&self, rid: ResourceId) -> bool {
    self.index.contains_key(&rid)
  }

  /// Returns a reference counted pointer to the resource of type `T` with the
  /// given `rid`. If `rid` is not present or has a type different than `T`,
  /// this function returns `None`.
  pub fn get<T: Resource>(
    &self,
    rid: ResourceId,
  ) -> Result<Rc<T>, ResourceError> {
    self
      .index
      .get(&rid)
      .and_then(|rc| rc.downcast_rc::<T>())
      .cloned()
      .ok_or(ResourceError::BadResourceId)
  }

  pub fn get_any(
    &self,
    rid: ResourceId,
  ) -> Result<Rc<dyn Resource>, ResourceError> {
    self
      .index
      .get(&rid)
      .cloned()
      .ok_or(ResourceError::BadResourceId)
  }

  /// Replaces a resource with a new resource.
  ///
  /// Panics if the resource does not exist.
  pub fn replace<T: Resource>(&mut self, rid: ResourceId, resource: T) {
    let result = self
      .index
      .insert(rid, Rc::new(resource) as Rc<dyn Resource>);
    assert!(result.is_some());
  }

  /// Removes a resource of type `T` from the resource table and returns it.
  /// If a resource with the given `rid` exists but its type does not match `T`,
  /// it is not removed from the resource table. Note that the resource's
  /// `close()` method is *not* called.
  ///
  /// Also note that there might be a case where
  /// the returned `Rc<T>` is referenced by other variables. That is, we cannot
  /// assume that `Rc::strong_count(&returned_rc)` is always equal to 1 on success.
  /// In particular, be really careful when you want to extract the inner value of
  /// type `T` from `Rc<T>`.
  pub fn take<T: Resource>(
    &mut self,
    rid: ResourceId,
  ) -> Result<Rc<T>, ResourceError> {
    let resource = self.get::<T>(rid)?;
    self.index.remove(&rid);
    Ok(resource)
  }

  /// Removes a resource from the resource table and returns it. Note that the
  /// resource's `close()` method is *not* called.
  ///
  /// Also note that there might be a
  /// case where the returned `Rc<T>` is referenced by other variables. That is,
  /// we cannot assume that `Rc::strong_count(&returned_rc)` is always equal to 1
  /// on success. In particular, be really careful when you want to extract the
  /// inner value of type `T` from `Rc<T>`.
  pub fn take_any(
    &mut self,
    rid: ResourceId,
  ) -> Result<Rc<dyn Resource>, ResourceError> {
    self.index.remove(&rid).ok_or(ResourceError::BadResourceId)
  }

  /// Removes the resource with the given `rid` from the resource table. If the
  /// only reference to this resource existed in the resource table, this will
  /// cause the resource to be dropped. However, since resources are reference
  /// counted, therefore pending ops are not automatically cancelled. A resource
  /// may implement the `close()` method to perform clean-ups such as canceling
  /// ops.
  #[deprecated = "This method may deadlock. Use take() and close() instead."]
  pub fn close(&mut self, rid: ResourceId) -> Result<(), ResourceError> {
    self
      .index
      .remove(&rid)
      .ok_or(ResourceError::BadResourceId)
      .map(|resource| resource.close())
  }

  /// Returns an iterator that yields a `(id, name)` pair for every resource
  /// that's currently in the resource table. This can be used for debugging
  /// purposes or to implement the `op_resources` op. Note that the order in
  /// which items appear is not specified.
  ///
  /// # Example
  ///
  /// ```
  /// # use deno_core::ResourceTable;
  /// # let resource_table = ResourceTable::default();
  /// let resource_names = resource_table.names().collect::<Vec<_>>();
  /// ```
  pub fn names(&self) -> impl Iterator<Item = (ResourceId, Cow<'_, str>)> {
    self
      .index
      .iter()
      .map(|(&id, resource)| (id, resource.name()))
  }

  /// Retrieves the [`ResourceHandleFd`] for a given resource, for potential optimization
  /// purposes within ops.
  pub fn get_fd(
    &self,
    rid: ResourceId,
  ) -> Result<ResourceHandleFd, ResourceError> {
    let Some(handle) = self.get_any(rid)?.backing_handle() else {
      return Err(ResourceError::BadResourceId);
    };
    let Some(fd) = handle.as_fd_like() else {
      return Err(ResourceError::BadResourceId);
    };
    if !handle.is_valid() {
      return Err(ResourceError::Reference);
    }
    Ok(fd)
  }

  /// Retrieves the [`ResourceHandleSocket`] for a given resource, for potential optimization
  /// purposes within ops.
  pub fn get_socket(
    &self,
    rid: ResourceId,
  ) -> Result<ResourceHandleSocket, ResourceError> {
    let Some(handle) = self.get_any(rid)?.backing_handle() else {
      return Err(ResourceError::BadResourceId);
    };
    let Some(socket) = handle.as_socket_like() else {
      return Err(ResourceError::BadResourceId);
    };
    if !handle.is_valid() {
      return Err(ResourceError::Reference);
    }
    Ok(socket)
  }

  /// Retrieves the [`ResourceHandle`] for a given resource, for potential optimization
  /// purposes within ops.
  pub fn get_handle(
    &self,
    rid: ResourceId,
  ) -> Result<ResourceHandle, ResourceError> {
    let Some(handle) = self.get_any(rid)?.backing_handle() else {
      return Err(ResourceError::BadResourceId);
    };
    if !handle.is_valid() {
      return Err(ResourceError::Reference);
    }
    Ok(handle)
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ResourceError {
  #[class(reference)]
  #[error("null or invalid handle")]
  Reference,
  #[class("BadResource")]
  #[error("Bad resource ID")]
  BadResourceId,
  #[class("Busy")]
  #[error("Resource is unavailable because it is in use by a promise")]
  Unavailable,
  #[class("BadResource")]
  #[error("{0}")]
  Other(String),
}
