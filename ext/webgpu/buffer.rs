// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::futures::channel::oneshot;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_error::JsErrorBox;
use wgpu_core::device::HostMap as MapMode;

use crate::Instance;
use crate::error::GPUGenericError;

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUBufferDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub size: u64,
  #[options(enforce_range = true)]
  pub usage: u32,
  #[webidl(default = false)]
  pub mapped_at_creation: bool,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum BufferError {
  #[class(generic)]
  #[error(transparent)]
  Canceled(#[from] oneshot::Canceled),
  #[class("DOMExceptionOperationError")]
  #[error(transparent)]
  Access(#[from] wgpu_core::resource::BufferAccessError),
  #[class("DOMExceptionOperationError")]
  #[error("{0}")]
  Operation(&'static str),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

pub struct GPUBuffer {
  pub instance: Instance,
  pub error_handler: super::error::ErrorHandler,

  pub id: wgpu_core::id::BufferId,
  pub device: wgpu_core::id::DeviceId,

  pub label: String,

  pub size: u64,
  pub usage: u32,

  pub map_state: RefCell<&'static str>,
  pub map_mode: RefCell<Option<MapMode>>,

  pub mapped_js_buffers: RefCell<Vec<MappedJsBuffer>>,
}

pub struct MappedJsBuffer {
  pub buffer: v8::Global<v8::ArrayBuffer>,
  pub offset: u64,
  pub size: u64,
  pub copy_on_unmap: bool,
}

impl Drop for GPUBuffer {
  fn drop(&mut self) {
    self.instance.buffer_drop(self.id);
  }
}

impl GPUBuffer {
  fn detach_mapped_js_buffers(&self, scope: &mut v8::PinScope<'_, '_>) {
    for mapped in self.mapped_js_buffers.replace(vec![]) {
      let ab = mapped.buffer.open(scope);
      ab.detach(None);
    }
  }

  fn writeback_and_detach_mapped_js_buffers(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
  ) -> Result<(), BufferError> {
    let mut first_error = None;
    for mapped in self.mapped_js_buffers.replace(vec![]) {
      let ab = mapped.buffer.open(scope);
      // A zero `byte_length` means the range is already detached, which can
      // only happen if the caller detached it themselves (e.g. by transferring
      // it to a worker). There is nothing left to read from, so the range is
      // skipped: any writes made through a transferred copy are not propagated
      // to the buffer. That matches the mapping model, where the range handed
      // out by `getMappedRange()` is the only view that stays valid until
      // `unmap()`.
      if mapped.copy_on_unmap && mapped.size != 0 && ab.byte_length() != 0 {
        match self.instance.buffer_get_mapped_range(
          self.id,
          mapped.offset,
          Some(mapped.size),
        ) {
          Ok((dst, _)) => {
            if let Some(src) = ab.data() {
              // SAFETY: `src` points to this V8-owned ArrayBuffer's backing
              // store, and `dst`/`mapped.size` were revalidated by wgpu for
              // the currently mapped range.
              unsafe {
                std::ptr::copy_nonoverlapping(
                  src.as_ptr() as *const u8,
                  dst.as_ptr(),
                  mapped.size as usize,
                );
              }
            }
          }
          Err(err) => {
            first_error.get_or_insert(BufferError::Access(err));
          }
        }
      }
      ab.detach(None);
    }

    if let Some(err) = first_error {
      return Err(err);
    }
    Ok(())
  }
}

impl WebIdlInterfaceConverter for GPUBuffer {
  const NAME: &'static str = "GPUBuffer";
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUBuffer {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUBuffer"
  }
}

#[op2]
impl GPUBuffer {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPUBuffer, GPUGenericError> {
    Err(GPUGenericError::InvalidConstructor)
  }

  #[getter]
  #[string]
  fn label(&self) -> String {
    self.label.clone()
  }
  #[setter]
  #[string]
  fn label(&self, #[webidl] _label: String) {
    // TODO(@crowlKats): no-op, needs wpgu to implement changing the label
  }

  #[getter]
  #[number]
  fn size(&self) -> u64 {
    self.size
  }
  #[getter]
  fn usage(&self) -> u32 {
    self.usage
  }

  #[getter]
  #[string]
  fn map_state(&self) -> &'static str {
    *self.map_state.borrow()
  }

  // In the successful case, the promise should resolve to undefined, but
  // `#[undefined]` does not seem to work here.
  // https://github.com/denoland/deno/issues/29603
  async fn map_async(
    &self,
    #[webidl(options(enforce_range = true))] mode: u32,
    #[webidl(default = 0)] offset: u64,
    #[webidl] size: Option<u64>,
  ) -> Result<(), BufferError> {
    let read_mode = (mode & 0x0001) == 0x0001;
    let write_mode = (mode & 0x0002) == 0x0002;
    if (read_mode && write_mode) || (!read_mode && !write_mode) {
      return Err(BufferError::Operation(
        "exactly one of READ or WRITE map mode must be set",
      ));
    }

    let mode = if read_mode {
      MapMode::Read
    } else {
      assert!(write_mode);
      MapMode::Write
    };

    {
      *self.map_state.borrow_mut() = "pending";
    }

    let (sender, receiver) =
      oneshot::channel::<wgpu_core::resource::BufferAccessResult>();

    {
      let callback = Box::new(move |status| {
        sender.send(status).unwrap();
      });

      let err = self
        .instance
        .buffer_map_async(
          self.id,
          offset,
          size,
          wgpu_core::resource::BufferMapOperation {
            host: mode,
            callback: Some(callback),
          },
        )
        .err();

      if err.is_some() {
        self.error_handler.push_error(err);
        return Err(BufferError::Operation("validation error occurred"));
      }
    }

    let done = Rc::new(RefCell::new(false));
    let done_ = done.clone();
    let device_poll_fut = async move {
      while !*done.borrow() {
        {
          self
            .instance
            .device_poll(self.device, wgpu_types::PollType::wait_indefinitely())
            .unwrap();
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
      }
      Ok::<(), BufferError>(())
    };

    let receiver_fut = async move {
      receiver.await??;
      let mut done = done_.borrow_mut();
      *done = true;
      Ok::<(), BufferError>(())
    };

    tokio::try_join!(device_poll_fut, receiver_fut)?;

    *self.map_state.borrow_mut() = "mapped";
    *self.map_mode.borrow_mut() = Some(mode);

    Ok(())
  }

  fn get_mapped_range<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    #[webidl(default = 0)] offset: u64,
    #[webidl] size: Option<u64>,
  ) -> Result<v8::Local<'s, v8::ArrayBuffer>, BufferError> {
    let (slice_pointer, range_size) = self
      .instance
      .buffer_get_mapped_range(self.id, offset, size)
      .map_err(BufferError::Access)?;

    let mode = self.map_mode.borrow();
    let mode = mode.as_ref().unwrap();

    // SAFETY: creating a slice from the pointer and length provided by wgpu.
    // Copy into a V8-owned backing store so a later GPUBuffer.destroy(), drop,
    // or backend unmap cannot leave JS holding a pointer to freed native memory.
    let slice = unsafe {
      std::slice::from_raw_parts(slice_pointer.as_ptr(), range_size as usize)
    };
    let bs = v8::ArrayBuffer::new_backing_store_from_vec(slice.to_vec());

    let shared_bs = bs.make_shared();
    let ab = v8::ArrayBuffer::with_backing_store(scope, &shared_bs);

    self.mapped_js_buffers.borrow_mut().push(MappedJsBuffer {
      buffer: v8::Global::new(scope, ab),
      offset,
      size: range_size,
      copy_on_unmap: mode == &MapMode::Write,
    });

    Ok(ab)
  }

  #[nofast]
  #[undefined]
  fn unmap(&self, scope: &mut v8::PinScope<'_, '_>) -> Result<(), BufferError> {
    // Writeback has to happen while the backend mapping is still live, so it
    // runs before `buffer_unmap()`. If it fails we bail out and leave
    // `map_state` as "mapped", matching the previous behavior where a failing
    // `buffer_unmap()` left the state untouched. In practice this is
    // unreachable: the range was validated by `getMappedRange()` and the
    // mapping is still active here.
    self.writeback_and_detach_mapped_js_buffers(scope)?;

    self
      .instance
      .buffer_unmap(self.id)
      .map_err(BufferError::Access)?;

    *self.map_state.borrow_mut() = "unmapped";

    Ok(())
  }

  #[nofast]
  #[undefined]
  fn destroy(&self, scope: &mut v8::PinScope<'_, '_>) {
    self.detach_mapped_js_buffers(scope);
    self.instance.buffer_destroy(self.id);
  }
}
