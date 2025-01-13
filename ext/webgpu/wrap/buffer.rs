// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::Duration;

use deno_core::futures::channel::oneshot;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_error::JsErrorBox;
use wgpu_core::resource::BufferMapCallback;

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

  pub mapped_js_buffers:
    RefCell<Vec<(NonNull<u8>, usize, Option<v8::Global<v8::Uint8Array>>)>>,
}

impl WebIdlInterfaceConverter for GPUBuffer {
  const NAME: &'static str = "GPUBuffer";
}

impl GarbageCollected for GPUBuffer {}

#[op2]
impl GPUBuffer {
  crate::with_label!();

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
    #[webidl/*(options(enforce_range = true))*/] mode: u32,
    #[webidl/*(default = 0)*/] offset: u64,
    #[webidl] size: Option<u64>,
  ) -> Result<(), BufferError> {
    let range_size = size.unwrap_or_else(|| self.size.saturating_sub(offset));
    if (offset % 8) != 0 {
      /*
       throw new DOMException(
         `${prefix}: offset must be a multiple of 8, received ${offset}`,
         "OperationError",
       );
      */
    }
    if (range_size % 4) != 0 {
      /*
       throw new DOMException(
         `${prefix}: rangeSize must be a multiple of 4, received ${rangeSize}`,
         "OperationError",
       );
      */
    }
    if (offset + range_size) > self.size {
      /*
      throw new DOMException(
        `${prefix}: offset + rangeSize must be less than or equal to buffer size`,
        "OperationError",
      );
       */
    }

    let read_mode = (mode & 0x0001) == 0x0001;
    let write_mode = (mode & 0x0002) == 0x0002;
    if (read_mode && write_mode) || (!read_mode && !write_mode) {
      /*
       throw new DOMException(
         `${prefix}: exactly one of READ or WRITE map mode must be set`,
         "OperationError",
       );
      */
    }

    if read_mode && !(self.usage & 0x0001) == 0x0001 {
      /*
       throw new DOMException(
         `${prefix}: READ map mode not valid because buffer does not have MAP_READ usage`,
         "OperationError",
       );
      */
    }

    if write_mode && !(self.usage & 0x0002) == 0x0002 {
      /*
       throw new DOMException(
         `${prefix}: WRITE map mode not valid because buffer does not have MAP_WRITE usage`,
         "OperationError",
       );
      */
    }

    {
      *self.map_state.borrow_mut() = "pending";
    }

    let (sender, receiver) =
      oneshot::channel::<wgpu_core::resource::BufferAccessResult>();

    {
      let callback = Box::new(move |status| {
        sender.send(status).unwrap();
      });

      // TODO(lucacasonato): error handling
      let err = self
        .instance
        .buffer_map_async(
          self.id,
          offset,
          Some(range_size),
          wgpu_core::resource::BufferMapOperation {
            host: if read_mode {
              wgpu_core::device::HostMap::Read
            } else {
              assert!(write_mode);
              wgpu_core::device::HostMap::Write
            },
            callback: Some(BufferMapCallback::from_rust(callback)),
          },
        )
        .err();

      if err.is_some() {
        self.error_handler.push_error(err);
        return Err(
          JsErrorBox::new(
            "DOMExceptionOperationError",
            "validation error occurred",
          )
          .into(),
        );
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

    Ok(())
  }

  #[buffer]
  fn get_mapped_range(
    &self,
    scope: &mut v8::HandleScope,
    #[webidl/*(default = 0)*/] offset: u64,
    #[webidl] size: Option<u64>,
  ) -> Result<Vec<u8>, BufferError> {
    let size = size.unwrap_or_else(|| self.size.saturating_sub(offset));

    let (slice_pointer, range_size) = self
      .instance
      .buffer_get_mapped_range(self.id, offset, Some(size))
      .map_err(BufferError::Access)?;

    let slice = unsafe {
      std::slice::from_raw_parts(slice_pointer.as_ptr(), range_size as usize)
    };
    let ab = v8::ArrayBuffer::new(scope, slice.len());
    v8::Uint8Array::new(scope, ab, 0, slice.len());

    // TODO: store buf
    self
      .mapped_js_buffers
      .borrow_mut()
      .push((slice_pointer, range_size as usize));

    Ok(slice.to_vec())
  }

  #[nofast]
  fn unmap(&self, scope: &mut v8::HandleScope) -> Result<(), BufferError> {
    for (slice_pointer, range_size, buf) in
      self.mapped_js_buffers.replace(vec![])
    {
      if let Some(buf) = buf {
        let buf = buf.open(scope);

        let slice = unsafe {
          std::slice::from_raw_parts_mut(slice_pointer.as_ptr(), range_size)
        };
        buf.copy_contents(slice);
      }
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
