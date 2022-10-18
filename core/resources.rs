// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated
// by the privileged side of Deno which refer to various rust objects that need
// to be persisted between various ops. For example, network sockets are
// resources. Resources may or may not correspond to a real operating system
// file descriptor (hence the different name).

use crate::error::bad_resource_id;
use crate::error::not_supported;
use crate::io::BufMutView;
use crate::io::BufView;
use crate::io::WriteOutcome;
use anyhow::Error;
use futures::Future;
use std::any::type_name;
use std::any::Any;
use std::any::TypeId;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::iter::Iterator;
use std::pin::Pin;
use std::rc::Rc;

/// Returned by resource read/write/shutdown methods
pub type AsyncResult<T> = Pin<Box<dyn Future<Output = Result<T, Error>>>>;

/// Resources are Rust objects that are attached to a [deno_core::JsRuntime].
/// They are identified in JS by a numeric ID (the resource ID, or rid).
/// Resources can be created in ops. Resources can also be retrieved in ops by
/// their rid. Resources are not thread-safe - they can only be accessed from
/// the thread that the JsRuntime lives on.
///
/// Resources are reference counted in Rust. This means that they can be
/// cloned and passed around. When the last reference is dropped, the resource
/// is automatically closed. As long as the resource exists in the resource
/// table, the reference count is at least 1.
///
/// ### Readable
///
/// Readable resources are resources that can have data read from. Examples of
/// this are files, sockets, or HTTP streams.
///
/// Readables can be read from from either JS or Rust. In JS one can use
/// `Deno.core.read()` to read from a single chunk of data from a readable. In
/// Rust one can directly call `read()` or `read_byob()`. The Rust side code is
/// used to implement ops like `op_slice`.
///
/// A distinction can be made between readables that produce chunks of data
/// themselves (they allocate the chunks), and readables that fill up
/// bring-your-own-buffers (BYOBs). The former is often the case for framed
/// protocols like HTTP, while the latter is often the case for kernel backed
/// resources like files and sockets.
///
/// All readables must implement `read()`. If resources can support an optimized
/// path for BYOBs, they should also implement `read_byob()`. For kernel backed
/// resources it often makes sense to implement `read_byob()` first, and then
/// implement `read()` as an operation that allocates a new chunk with
/// `len == limit`, then calls `read_byob()`, and then returns a chunk sliced to
/// the number of bytes read. Kernel backed resources can use the
/// [deno_core::impl_readable_byob] macro to implement optimized `read_byob()`
/// and `read()` implementations from a single `Self::read()` method.
///
/// ### Writable
///
/// Writable resources are resources that can have data written to. Examples of
/// this are files, sockets, or HTTP streams.
///
/// Writables can be written to from either JS or Rust. In JS one can use
/// `Deno.core.write()` to write to a single chunk of data to a writable. In
/// Rust one can directly call `write()`. The latter is used to implement ops
/// like `op_slice`.
pub trait Resource: Any + 'static {
  /// Returns a string representation of the resource which is made available
  /// to JavaScript code through `op_resources`. The default implementation
  /// returns the Rust type name, but specific resource types may override this
  /// trait method.
  fn name(&self) -> Cow<str> {
    type_name::<Self>().into()
  }

  /// Read a single chunk of data from the resource. This operation returns a
  /// `BufView` that represents the data that was read. If a zero length buffer
  /// is returned, it indicates that the resource has reached EOF.
  ///
  /// If this method is not implemented, the default implementation will error
  /// with a "not supported" error.
  ///
  /// If a readable can provide an optimized path for BYOBs, it should also
  /// implement `read_byob()`.
  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    _ = limit;
    Box::pin(futures::future::err(not_supported()))
  }

  /// Read a single chunk of data from the resource into the provided `BufMutView`.
  ///
  /// This operation returns the number of bytes read. If zero bytes are read,
  /// it indicates that the resource has reached EOF.
  ///
  /// If this method is not implemented explicitly, the default implementation
  /// will call `read()` and then copy the data into the provided buffer. For
  /// readable resources that can provide an optimized path for BYOBs, it is
  /// strongly recommended to override this method.
  fn read_byob(
    self: Rc<Self>,
    mut buf: BufMutView,
  ) -> AsyncResult<(usize, BufMutView)> {
    Box::pin(async move {
      let read = self.read(buf.len()).await?;
      let nread = read.len();
      buf[..nread].copy_from_slice(&read);
      Ok((nread, buf))
    })
  }

  /// Write a single chunk of data to the resource. The operation may not be
  /// able to write the entire chunk, in which case it should return the number
  /// of bytes written. Additionally it should return the `BufView` that was
  /// passed in.
  ///
  /// If this method is not implemented, the default implementation will error
  /// with a "not supported" error.
  fn write(self: Rc<Self>, buf: BufView) -> AsyncResult<WriteOutcome> {
    _ = buf;
    Box::pin(futures::future::err(not_supported()))
  }

  /// Write an entire chunk of data to the resource. Unlike `write()`, this will
  /// ensure the entire chunk is written. If the operation is not able to write
  /// the entire chunk, an error is to be returned.
  ///
  /// By default this method will call `write()` repeatedly until the entire
  /// chunk is written. Resources that can write the entire chunk in a single
  /// operation using an optimized path should override this method.
  fn write_all(self: Rc<Self>, view: BufView) -> AsyncResult<()> {
    Box::pin(async move {
      let mut view = view;
      let this = self;
      while !view.is_empty() {
        let resp = this.clone().write(view).await?;
        match resp {
          WriteOutcome::Partial {
            nwritten,
            view: new_view,
          } => {
            view = new_view;
            view.advance_cursor(nwritten);
          }
          WriteOutcome::Full { .. } => break,
        }
      }
      Ok(())
    })
  }

  /// The shutdown method can be used to asynchronously close the resource. It
  /// is not automatically called when the resource is dropped or closed.
  ///
  /// If this method is not implemented, the default implementation will error
  /// with a "not supported" error.
  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(futures::future::err(not_supported()))
  }

  /// Resources may implement the `close()` trait method if they need to do
  /// resource specific clean-ups, such as cancelling pending futures, after a
  /// resource has been removed from the resource table.
  fn close(self: Rc<Self>) {}

  /// Resources backed by a file descriptor can let ops know to allow for
  /// low-level optimizations.
  #[cfg(unix)]
  fn backing_fd(self: Rc<Self>) -> Option<std::os::unix::prelude::RawFd> {
    None
  }

  fn size_hint(&self) -> (u64, Option<u64>) {
    (0, None)
  }
}

impl dyn Resource {
  #[inline(always)]
  fn is<T: Resource>(&self) -> bool {
    self.type_id() == TypeId::of::<T>()
  }

  #[inline(always)]
  #[allow(clippy::needless_lifetimes)]
  pub fn downcast_rc<'a, T: Resource>(self: &'a Rc<Self>) -> Option<&'a Rc<T>> {
    if self.is::<T>() {
      let ptr = self as *const Rc<_> as *const Rc<T>;
      // TODO(piscisaureus): safety comment
      #[allow(clippy::undocumented_unsafe_blocks)]
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
  pub fn get<T: Resource>(&self, rid: ResourceId) -> Result<Rc<T>, Error> {
    self
      .index
      .get(&rid)
      .and_then(|rc| rc.downcast_rc::<T>())
      .map(Clone::clone)
      .ok_or_else(bad_resource_id)
  }

  pub fn get_any(&self, rid: ResourceId) -> Result<Rc<dyn Resource>, Error> {
    self
      .index
      .get(&rid)
      .map(Clone::clone)
      .ok_or_else(bad_resource_id)
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
  pub fn take<T: Resource>(&mut self, rid: ResourceId) -> Result<Rc<T>, Error> {
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
  ) -> Result<Rc<dyn Resource>, Error> {
    self.index.remove(&rid).ok_or_else(bad_resource_id)
  }

  /// Removes the resource with the given `rid` from the resource table. If the
  /// only reference to this resource existed in the resource table, this will
  /// cause the resource to be dropped. However, since resources are reference
  /// counted, therefore pending ops are not automatically cancelled. A resource
  /// may implement the `close()` method to perform clean-ups such as canceling
  /// ops.
  pub fn close(&mut self, rid: ResourceId) -> Result<(), Error> {
    self
      .index
      .remove(&rid)
      .ok_or_else(bad_resource_id)
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
  pub fn names(&self) -> impl Iterator<Item = (ResourceId, Cow<str>)> {
    self
      .index
      .iter()
      .map(|(&id, resource)| (id, resource.name()))
  }
}

#[macro_export]
macro_rules! impl_readable_byob {
  () => {
    fn read(self: Rc<Self>, limit: usize) -> AsyncResult<$crate::BufView> {
      Box::pin(async move {
        let mut vec = vec![0; limit];
        let nread = self.read(&mut vec).await?;
        if nread != vec.len() {
          vec.truncate(nread);
        }
        let view = $crate::BufView::from(vec);
        Ok(view)
      })
    }

    fn read_byob(
      self: Rc<Self>,
      mut buf: $crate::BufMutView,
    ) -> AsyncResult<(usize, $crate::BufMutView)> {
      Box::pin(async move {
        let nread = self.read(buf.as_mut()).await?;
        Ok((nread, buf))
      })
    }
  };
}

#[macro_export]
macro_rules! impl_writable {
  (__write) => {
    fn write(
      self: Rc<Self>,
      view: $crate::BufView,
    ) -> AsyncResult<$crate::WriteOutcome> {
      Box::pin(async move {
        let nwritten = self.write(&view).await?;
        Ok($crate::WriteOutcome::Partial { nwritten, view })
      })
    }
  };
  (__write_all) => {
    fn write_all(self: Rc<Self>, view: $crate::BufView) -> AsyncResult<()> {
      Box::pin(async move {
        self.write_all(&view).await?;
        Ok(())
      })
    }
  };
  () => {
    $crate::impl_writable!(__write);
  };
  (with_all) => {
    $crate::impl_writable!(__write);
    $crate::impl_writable!(__write_all);
  };
}
