// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use deno_core::futures::channel::oneshot;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_error::JsErrorBox;
use wgpu_core::device::HostMap as MapMode;

use crate::Instance;

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

  pub mapped_js_buffers: RefCell<Vec<v8::Global<v8::ArrayBuffer>>>,
}

impl Drop for GPUBuffer {
  fn drop(&mut self) {
    self.instance.buffer_drop(self.id);
  }
}

impl WebIdlInterfaceConverter for GPUBuffer {
  const NAME: &'static str = "GPUBuffer";
}

impl GarbageCollected for GPUBuffer {}

#[op2]
impl GPUBuffer {
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

  #[async_method]
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
            .device_poll(self.device, wgpu_types::Maintain::wait())
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
    scope: &mut v8::HandleScope<'s>,
    #[webidl(default = 0)] offset: u64,
    #[webidl] size: Option<u64>,
  ) -> Result<v8::Local<'s, v8::ArrayBuffer>, BufferError> {
    let (slice_pointer, range_size) = self
      .instance
      .buffer_get_mapped_range(self.id, offset, size)
      .map_err(BufferError::Access)?;

    let mode = self.map_mode.borrow();
    let mode = mode.as_ref().unwrap();

    let bs = if mode == &MapMode::Write {
      unsafe extern "C" fn noop_deleter_callback(
        _data: *mut std::ffi::c_void,
        _byte_length: usize,
        _deleter_data: *mut std::ffi::c_void,
      ) {
      }

      // SAFETY: creating a backing store from the pointer and length provided by wgpu
      unsafe {
        v8::ArrayBuffer::new_backing_store_from_ptr(
          slice_pointer.as_ptr() as _,
          range_size as usize,
          noop_deleter_callback,
          std::ptr::null_mut(),
        )
      }
    } else {
      // SAFETY: creating a vector from the pointer and length provided by wgpu
      let slice = unsafe {
        std::slice::from_raw_parts(slice_pointer.as_ptr(), range_size as usize)
      };
      v8::ArrayBuffer::new_backing_store_from_vec(slice.to_vec())
    };

    let shared_bs = bs.make_shared();
    let ab = v8::ArrayBuffer::with_backing_store(scope, &shared_bs);

    if mode == &MapMode::Write {
      self
        .mapped_js_buffers
        .borrow_mut()
        .push(v8::Global::new(scope, ab));
    }

    Ok(ab)
  }

  #[nofast]
  fn unmap(&self, scope: &mut v8::HandleScope) -> Result<(), BufferError> {
    for ab in self.mapped_js_buffers.replace(vec![]) {
      let ab = ab.open(scope);
      ab.detach(None);
    }

    self
      .instance
      .buffer_unmap(self.id)
      .map_err(BufferError::Access)?;

    *self.map_state.borrow_mut() = "unmapped";

    Ok(())
  }

  #[fast]
  fn destroy(&self) -> Result<(), JsErrorBox> {
    self
      .instance
      .buffer_destroy(self.id)
      .map_err(|e| JsErrorBox::generic(e.to_string()))
  }
}
