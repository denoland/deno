// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::rc::Rc;

use super::Resource;
use super::ResourceHandle;
use super::ResourceHandleFd;
use super::ResourceHandleSocket;

/// A `ResourceId` is an integer value referencing a resource. It could be
/// considered to be the Deno equivalent of a `file descriptor` in POSIX like
/// operating systems. Elsewhere in the code base it is commonly abbreviated
/// to `rid`.
///
/// A rid is not an opaque counter: it packs a slot index into the resource
/// table together with a generation counter for that slot (see
/// [`ResourceTable`]). Nothing outside the table may interpret those bits.
// TODO: use `u64` instead?
pub type ResourceId = u32;

/// Number of low bits of a [`ResourceId`] holding the slot index.
///
/// Together with [`GENERATION_BITS`] this must stay at or below 31 so that
/// every rid is a positive JS smi: ops receive rids as `#[smi]`, and the
/// leak-sanitizer stats keep rid-indexed sets.
const INDEX_BITS: u32 = 20;
/// Number of bits above the index holding the slot's generation.
const GENERATION_BITS: u32 = 11;
const INDEX_MASK: u32 = (1 << INDEX_BITS) - 1;
const GENERATION_MASK: u32 = (1 << GENERATION_BITS) - 1;
/// Maximum number of slots (live or free) the table can hold.
const MAX_SLOTS: usize = 1 << INDEX_BITS;

#[inline(always)]
const fn pack(index: u32, generation: u32) -> ResourceId {
  (generation << INDEX_BITS) | index
}

#[inline(always)]
const fn unpack(rid: ResourceId) -> (usize, u32) {
  ((rid & INDEX_MASK) as usize, rid >> INDEX_BITS)
}

struct Slot {
  resource: Option<Rc<dyn Resource>>,
  /// Incremented every time this slot is vacated, so that a stale rid does
  /// not resolve to whatever resource occupies the slot next.
  generation: u32,
}

/// Map-like data structure storing Deno's resources (equivalent to file
/// descriptors).
///
/// Provides basic methods for element access. A resource can be of any type.
/// Different types of resources can be stored in the same map, and provided
/// with a name for description.
///
/// Each resource is identified through a _resource ID (rid)_, which acts as
/// the key in the map.
///
/// # Resource ids
///
/// The table is a slab: a rid is a slot index (low 20 bits) plus that slot's
/// generation (next 11 bits), so lookup is a bounds check and an index rather
/// than a tree walk. Two consequences, both deliberate:
///
/// - **Ids are reused.** Closing a resource returns its slot to a free list,
///   and a later `add` may hand out a rid whose index is the same. The
///   generation counter means the *exact* rid value is only recycled after
///   that slot has been vacated 2^11 times, so a stale rid is rejected rather
///   than silently resolving to an unrelated resource. A great deal of code
///   above this layer holds rids indefinitely and treats "bad resource" as
///   proof the resource is gone; the generation is what keeps that idiom
///   sound.
/// - **Ids are not monotonic.** They are still handed out in increasing order
///   as long as nothing has been closed, so the first resources added to a
///   fresh table get 0, 1, 2, ... exactly as before; but after a close, the
///   next rid may be lower than one already handed out.
///
/// Every rid fits in 31 bits and is therefore always a positive JS smi.
#[derive(Default)]
pub struct ResourceTable {
  slots: Vec<Slot>,
  /// Indices of vacant slots, most recently vacated first.
  free: Vec<u32>,
}

impl ResourceTable {
  /// Returns the number of resources currently active in the resource table.
  /// Resources taken from the table do not contribute to this count.
  pub fn len(&self) -> usize {
    self.slots.len() - self.free.len()
  }

  /// Returns whether this table is empty.
  pub fn is_empty(&self) -> bool {
    self.len() == 0
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
    if let Some(index) = self.free.pop() {
      let slot = &mut self.slots[index as usize];
      debug_assert!(slot.resource.is_none());
      slot.resource = Some(resource);
      pack(index, slot.generation)
    } else {
      let index = self.slots.len();
      assert!(index < MAX_SLOTS, "too many open resources");
      self.slots.push(Slot {
        resource: Some(resource),
        generation: 0,
      });
      pack(index as u32, 0)
    }
  }

  #[inline(always)]
  fn slot(&self, rid: ResourceId) -> Option<&Rc<dyn Resource>> {
    let (index, generation) = unpack(rid);
    let slot = self.slots.get(index)?;
    if slot.generation != generation {
      return None;
    }
    slot.resource.as_ref()
  }

  /// Vacates the slot addressed by `rid`, bumping its generation, and returns
  /// what was in it.
  #[inline]
  fn vacate(&mut self, rid: ResourceId) -> Option<Rc<dyn Resource>> {
    let (index, generation) = unpack(rid);
    let slot = self.slots.get_mut(index)?;
    if slot.generation != generation {
      return None;
    }
    let resource = slot.resource.take()?;
    slot.generation = (slot.generation + 1) & GENERATION_MASK;
    self.free.push(index as u32);
    Some(resource)
  }

  /// Returns true if any resource with the given `rid` exists.
  pub fn has(&self, rid: ResourceId) -> bool {
    self.slot(rid).is_some()
  }

  /// Returns a reference counted pointer to the resource of type `T` with the
  /// given `rid`. If `rid` is not present or has a type different than `T`,
  /// this function returns `None`.
  pub fn get<T: Resource>(
    &self,
    rid: ResourceId,
  ) -> Result<Rc<T>, ResourceError> {
    self
      .slot(rid)
      .and_then(|rc| rc.downcast_rc::<T>())
      .cloned()
      .ok_or(ResourceError::BadResourceId)
  }

  pub fn get_any(
    &self,
    rid: ResourceId,
  ) -> Result<Rc<dyn Resource>, ResourceError> {
    self.slot(rid).cloned().ok_or(ResourceError::BadResourceId)
  }

  /// Replaces a resource with a new resource.
  ///
  /// The `rid` is left unchanged, and so remains valid.
  ///
  /// Panics if the resource does not exist.
  pub fn replace<T: Resource>(&mut self, rid: ResourceId, resource: T) {
    let (index, generation) = unpack(rid);
    let slot = self
      .slots
      .get_mut(index)
      .filter(|slot| slot.generation == generation && slot.resource.is_some());
    let slot = slot.expect("attempted to replace a resource that is not open");
    slot.resource = Some(Rc::new(resource) as Rc<dyn Resource>);
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
    self.vacate(rid);
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
    self.vacate(rid).ok_or(ResourceError::BadResourceId)
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
      .vacate(rid)
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
    self.slots.iter().enumerate().filter_map(|(index, slot)| {
      let resource = slot.resource.as_ref()?;
      Some((pack(index as u32, slot.generation), resource.name()))
    })
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

#[cfg(test)]
mod tests {
  use std::borrow::Cow;

  use super::*;

  struct A;
  impl Resource for A {
    fn name(&self) -> Cow<'_, str> {
      "A".into()
    }
  }

  struct B;
  impl Resource for B {
    fn name(&self) -> Cow<'_, str> {
      "B".into()
    }
  }

  #[test]
  fn ids_start_dense_and_ascending() {
    let mut table = ResourceTable::default();
    assert!(table.is_empty());
    assert_eq!(table.add(A), 0);
    assert_eq!(table.add(A), 1);
    assert_eq!(table.add(A), 2);
    assert_eq!(table.len(), 3);
  }

  #[test]
  fn stale_rid_does_not_resolve_to_the_new_occupant() {
    let mut table = ResourceTable::default();
    let a = table.add(A);
    table.take::<A>(a).unwrap();
    assert!(!table.has(a));

    let b = table.add(B);
    // Same slot, different generation, so the ids differ...
    assert_ne!(a, b);
    // ...and the stale id resolves to nothing.
    assert!(!table.has(a));
    assert!(table.get::<B>(a).is_err());
    assert!(table.get_any(a).is_err());
    assert!(table.take_any(a).is_err());
    // ...while the live one still works.
    assert!(table.has(b));
    assert!(table.get::<B>(b).is_ok());
  }

  #[test]
  fn generation_wraps_and_slot_is_reused() {
    let mut table = ResourceTable::default();
    let mut seen = Vec::new();
    for _ in 0..(GENERATION_MASK + 1) {
      let rid = table.add(A);
      seen.push(rid);
      table.take::<A>(rid).unwrap();
    }
    // Every generation of the slot produced a distinct id...
    let mut sorted = seen.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), seen.len());
    // ...and after 2^GENERATION_BITS vacancies the id comes back around.
    assert_eq!(table.add(A), seen[0]);
    assert_eq!(table.slots.len(), 1);
  }

  #[test]
  fn take_of_wrong_type_leaves_the_resource_in_place() {
    let mut table = ResourceTable::default();
    let rid = table.add(A);
    assert!(table.take::<B>(rid).is_err());
    assert!(table.has(rid));
    assert_eq!(table.len(), 1);
    assert!(table.take::<A>(rid).is_ok());
    assert_eq!(table.len(), 0);
  }

  #[test]
  fn replace_keeps_the_rid_valid() {
    let mut table = ResourceTable::default();
    let rid = table.add(A);
    table.replace(rid, B);
    assert!(table.get::<A>(rid).is_err());
    assert!(table.get::<B>(rid).is_ok());
    assert_eq!(table.len(), 1);
  }

  #[test]
  #[should_panic(expected = "attempted to replace a resource that is not open")]
  fn replace_of_a_closed_rid_panics() {
    let mut table = ResourceTable::default();
    let rid = table.add(A);
    table.take::<A>(rid).unwrap();
    table.replace(rid, B);
  }

  #[test]
  fn names_lists_only_live_resources() {
    let mut table = ResourceTable::default();
    let a = table.add(A);
    let b = table.add(B);
    table.take::<A>(a).unwrap();
    let names = table.names().collect::<Vec<_>>();
    assert_eq!(names.len(), 1);
    assert_eq!(names[0].0, b);
    assert_eq!(names[0].1, "B");
  }

  #[test]
  fn out_of_range_rid_is_not_found() {
    let table = ResourceTable::default();
    assert!(!table.has(0));
    assert!(!table.has(u32::MAX));
    assert!(table.get_any(u32::MAX).is_err());
  }
}
