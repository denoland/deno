// Copyright 2018-2025 the Deno authors. MIT license.

// Think of Resources as File Descriptors. They are integers that are allocated
// by the privileged side of Deno which refer to various rust objects that need
// to be persisted between various ops. For example, network sockets are
// resources. Resources may or may not correspond to a real operating system
// file descriptor (hence the different name).

use crate::ResourceHandle;
use crate::ResourceHandleFd;
use crate::io::AsyncResult;
use crate::io::BufMutView;
use crate::io::BufView;
use crate::io::WriteOutcome;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use std::any::Any;
use std::any::TypeId;
use std::any::type_name;
use std::borrow::Cow;
use std::rc::Rc;

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
  fn name(&self) -> Cow<'_, str> {
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
    Box::pin(std::future::ready(Err(JsErrorBox::not_supported())))
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

  /// Write an error state to this resource, if the resource supports it.
  fn write_error(self: Rc<Self>, _error: &dyn JsErrorClass) -> AsyncResult<()> {
    Box::pin(std::future::ready(Err(JsErrorBox::not_supported())))
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
    Box::pin(std::future::ready(Err(JsErrorBox::not_supported())))
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

  /// The same as [`read_byob()`][Resource::read_byob], but synchronous.
  fn read_byob_sync(
    self: Rc<Self>,
    data: &mut [u8],
  ) -> Result<usize, JsErrorBox> {
    _ = data;
    Err(JsErrorBox::not_supported())
  }

  /// The same as [`write()`][Resource::write], but synchronous.
  fn write_sync(self: Rc<Self>, data: &[u8]) -> Result<usize, JsErrorBox> {
    _ = data;
    Err(JsErrorBox::not_supported())
  }

  /// The shutdown method can be used to asynchronously close the resource. It
  /// is not automatically called when the resource is dropped or closed.
  ///
  /// If this method is not implemented, the default implementation will error
  /// with a "not supported" error.
  fn shutdown(self: Rc<Self>) -> AsyncResult<()> {
    Box::pin(std::future::ready(Err(JsErrorBox::not_supported())))
  }

  /// Resources may implement the `close()` trait method if they need to do
  /// resource specific clean-ups, such as cancelling pending futures, after a
  /// resource has been removed from the resource table.
  fn close(self: Rc<Self>) {}

  /// Resources backed by a file descriptor or socket handle can let ops know
  /// to allow for low-level optimizations.
  fn backing_handle(self: Rc<Self>) -> Option<ResourceHandle> {
    #[allow(deprecated)]
    self.backing_fd().map(ResourceHandle::Fd)
  }

  /// Resources backed by a file descriptor can let ops know to allow for
  /// low-level optimizations.
  #[deprecated = "Use backing_handle"]
  fn backing_fd(self: Rc<Self>) -> Option<ResourceHandleFd> {
    None
  }

  fn size_hint(&self) -> (u64, Option<u64>) {
    (0, None)
  }

  fn transfer(
    self: Rc<Self>,
  ) -> Result<Box<dyn TransferredResource>, JsErrorBox> {
    Err(JsErrorBox::not_supported())
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

#[macro_export]
macro_rules! impl_readable_byob {
  () => {
    fn read(
      self: ::std::rc::Rc<Self>,
      limit: ::core::primitive::usize,
    ) -> AsyncResult<$crate::BufView> {
      ::std::boxed::Box::pin(async move {
        let mut vec = ::std::vec![0; limit];
        let nread = self.read(&mut vec).await.map_err(::deno_error::JsErrorBox::from_err)?;
        if nread != vec.len() {
          vec.truncate(nread);
        }
        let view = $crate::BufView::from(vec);
        ::std::result::Result::Ok(view)
      })
    }

    fn read_byob(
      self: ::std::rc::Rc<Self>,
      mut buf: $crate::BufMutView,
    ) -> AsyncResult<(::core::primitive::usize, $crate::BufMutView)> {
      ::std::boxed::Box::pin(async move {
        let nread = self.read(buf.as_mut()).await.map_err(::deno_error::JsErrorBox::from_err)?;
        ::std::result::Result::Ok((nread, buf))
      })
    }
  };
}

#[macro_export]
macro_rules! impl_writable {
  (__write) => {
    fn write(
      self: ::std::rc::Rc<Self>,
      view: $crate::BufView,
    ) -> $crate::AsyncResult<$crate::WriteOutcome> {
      ::std::boxed::Box::pin(async move {
        let nwritten = self
          .write(&view)
          .await
          .map_err(::deno_error::JsErrorBox::from_err)?;
        ::std::result::Result::Ok($crate::WriteOutcome::Partial {
          nwritten,
          view,
        })
      })
    }
  };
  (__write_all) => {
    fn write_all(
      self: ::std::rc::Rc<Self>,
      view: $crate::BufView,
    ) -> $crate::AsyncResult<()> {
      ::std::boxed::Box::pin(async move {
        self
          .write_all(&view)
          .await
          .map_err(::deno_error::JsErrorBox::from_err)?;
        ::std::result::Result::Ok(())
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

pub trait TransferredResource: Send {
  fn receive(self: Box<Self>) -> Rc<dyn Resource>;
}

impl dyn TransferredResource {}
