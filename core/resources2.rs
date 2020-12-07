// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated
// by the privileged side of Deno which refer to various rust objects that need
// to be persisted between various ops. For example, network sockets are
// resources. Resources may or may not correspond to a real operating system
// file descriptor (hence the different name).

use std::any::type_name;
use std::any::Any;
use std::any::TypeId;
use std::borrow::Cow;
use std::collections::HashMap;
use std::iter::Iterator;
use std::rc::Rc;

/// All objects that can be store in the resource table should implement the
/// `Resource` trait.
pub trait Resource: Any + 'static {
  /// Returns a string representation of the resource which is made available
  /// to JavaScript code through `op_resources`. The default implementation
  /// returns the Rust type name, but specific resource types may override this
  /// trait method.
  fn name(&self) -> Cow<str> {
    type_name::<Self>().into()
  }

  fn close(&mut self) {}
}

impl dyn Resource {
  #[inline(always)]
  fn is<T: Resource>(&self) -> bool {
    self.type_id() == TypeId::of::<T>()
  }

  #[inline(always)]
  #[allow(clippy::needless_lifetimes)]
  fn downcast_rc<'a, T: Resource>(self: &'a Rc<Self>) -> Option<&'a Rc<T>> {
    if self.is::<T>() {
      let ptr = self as *const Rc<_> as *const Rc<T>;
      Some(unsafe { &*ptr })
    } else {
      None
    }
  }
}

/// A `ResourceId` is an integer value referencing a resource. It could be
/// considered to be the Deno equivalent of a `file descriptor` in POSIX like
/// operating systems. Elsewhere in the code base it is commonly abbreviated
/// to `rid`.
// TODO: use `u64` instead?
pub type ResourceId = u32;

/// Temporary alias for `crate::resources2::ResourceTable`.
// TODO: remove this when the old `ResourceTable` is obsolete.
pub type ResourceTable2 = ResourceTable;

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
  index: HashMap<ResourceId, Rc<dyn Resource>>,
  next_rid: ResourceId,
}

impl ResourceTable {
  /// Returns true if any resource with the given `rid` is exists.
  pub fn has(&self, rid: ResourceId) -> bool {
    self.index.contains_key(&rid)
  }

  /// Returns a reference counted pointer to the resource of type `T` with the
  /// given `rid`. If `rid` is not present or has a type different than `T`,
  /// this function returns `None`.
  pub fn get<T: Resource>(&self, rid: ResourceId) -> Option<Rc<T>> {
    self
      .index
      .get(&rid)
      .and_then(|resource| resource.downcast_rc::<T>())
      .map(Clone::clone)
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
    let rid = self.next_rid;
    let removed_resource = self.index.insert(rid, resource);
    assert!(removed_resource.is_none());
    self.next_rid += 1;
    rid
  }

  /// Removes the resource with the given `rid` from the resource table. If the
  /// only reference to this resource existed in the resource table, this will
  /// cause the resource to be dropped. However, since resources are reference
  /// counted, therefore pending ops are not automatically cancelled.
  pub fn close(&mut self, rid: ResourceId) -> Option<()> {
    self.index.remove(&rid).map(|_| ())
  }

  /// Returns an iterator that yields a `(id, name)` pair for every resource
  /// that's currently in the resource table. This can be used for debugging
  /// purposes or to implement the `op_resources` op. Note that the order in
  /// which items appear is not specified.
  ///
  /// # Example
  ///
  /// ```
  /// # use deno_core::resources2::ResourceTable;
  /// # let resource_table = ResourceTable::default();
  /// let resource_names = resource_table.names().collect::<Vec<_>>();
  /// ```
  pub fn names(&self) -> impl Iterator<Item = (ResourceId, Cow<str>)> {
    self
      .index
      .iter()
      .map(|(&id, resource)| (id, resource.name()))
  }
}
