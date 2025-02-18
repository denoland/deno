// Copyright 2018-2025 the Deno authors. MIT license.

// NOTE(bartlomieju): unfortunately it appears that clippy is broken
// and can't allow a single line ignore for `await_holding_lock`.
#![allow(clippy::await_holding_lock)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::OpState;
use deno_error::JsErrorBox;
use jupyter_runtime::InputRequest;
use jupyter_runtime::JupyterMessage;
use jupyter_runtime::JupyterMessageContent;
use jupyter_runtime::KernelIoPubConnection;
use jupyter_runtime::StreamContent;
use tokio::sync::mpsc;

use crate::tools::jupyter::server::StdinConnectionProxy;

deno_core::extension!(deno_jupyter,
  ops = [
    op_jupyter_broadcast,
    op_jupyter_input,
    op_jupyter_create_png_from_texture,
    op_jupyter_get_buffer,
  ],
  options = {
    sender: mpsc::UnboundedSender<StreamContent>,
  },
  middleware = |op| match op.name {
    "op_print" => op_print(),
    _ => op,
  },
  state = |state, options| {
    state.put(options.sender);
  },
);

deno_core::extension!(deno_jupyter_for_test,
  ops = [
    op_jupyter_broadcast,
    op_jupyter_input,
    op_jupyter_create_png_from_texture,
    op_jupyter_get_buffer,
  ],
  options = {
    sender: mpsc::UnboundedSender<StreamContent>,
  },
  state = |state, options| {
    state.put(options.sender);
  },
);

#[op2]
#[string]
pub fn op_jupyter_input(
  state: &mut OpState,
  #[string] prompt: String,
  is_password: bool,
) -> Option<String> {
  let (last_execution_request, stdin_connection_proxy) = {
    (
      state.borrow::<Arc<Mutex<Option<JupyterMessage>>>>().clone(),
      state.borrow::<Arc<Mutex<StdinConnectionProxy>>>().clone(),
    )
  };

  let maybe_last_request = last_execution_request.lock().clone();
  if let Some(last_request) = maybe_last_request {
    let JupyterMessageContent::ExecuteRequest(msg) = &last_request.content
    else {
      return None;
    };

    if !msg.allow_stdin {
      return None;
    }

    let content = InputRequest {
      prompt,
      password: is_password,
    };

    let msg = JupyterMessage::new(content, Some(&last_request));

    let Ok(()) = stdin_connection_proxy.lock().tx.send(msg) else {
      return None;
    };

    // Need to spawn a separate thread here, because `blocking_recv()` can't
    // be used from the Tokio runtime context.
    let join_handle = std::thread::spawn(move || {
      stdin_connection_proxy.lock().rx.blocking_recv()
    });
    let Ok(Some(response)) = join_handle.join() else {
      return None;
    };

    let JupyterMessageContent::InputReply(msg) = response.content else {
      return None;
    };

    return Some(msg.value);
  }

  None
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum JupyterBroadcastError {
  #[class(inherit)]
  #[error(transparent)]
  SerdeJson(serde_json::Error),
  #[class(generic)]
  #[error(transparent)]
  ZeroMq(AnyError),
}

#[op2(async)]
pub async fn op_jupyter_broadcast(
  state: Rc<RefCell<OpState>>,
  #[string] message_type: String,
  #[serde] content: serde_json::Value,
  #[serde] metadata: serde_json::Value,
  #[serde] buffers: Vec<deno_core::JsBuffer>,
) -> Result<(), JupyterBroadcastError> {
  let (iopub_connection, last_execution_request) = {
    let s = state.borrow();

    (
      s.borrow::<Arc<Mutex<KernelIoPubConnection>>>().clone(),
      s.borrow::<Arc<Mutex<Option<JupyterMessage>>>>().clone(),
    )
  };

  let maybe_last_request = last_execution_request.lock().clone();
  if let Some(last_request) = maybe_last_request {
    let content = JupyterMessageContent::from_type_and_content(
      &message_type,
      content.clone(),
    )
    .map_err(|err| {
      log::error!(
          "Error deserializing content from jupyter.broadcast, message_type: {}:\n\n{}\n\n{}",
          &message_type,
          content,
          err
      );
      JupyterBroadcastError::SerdeJson(err)
    })?;

    let jupyter_message = JupyterMessage::new(content, Some(&last_request))
      .with_metadata(metadata)
      .with_buffers(buffers.into_iter().map(|b| b.to_vec().into()).collect());

    iopub_connection
      .lock()
      .send(jupyter_message)
      .await
      .map_err(JupyterBroadcastError::ZeroMq)?;
  }

  Ok(())
}

#[op2(fast)]
pub fn op_print(state: &mut OpState, #[string] msg: &str, is_err: bool) {
  let sender = state.borrow_mut::<mpsc::UnboundedSender<StreamContent>>();

  if is_err {
    if let Err(err) = sender.send(StreamContent::stderr(msg)) {
      log::error!("Failed to send stderr message: {}", err);
    }
    return;
  }

  if let Err(err) = sender.send(StreamContent::stdout(msg)) {
    log::error!("Failed to send stdout message: {}", err);
  }
}

#[op2]
#[string]
pub fn op_jupyter_create_png_from_texture(
  #[cppgc] texture: &deno_runtime::deno_webgpu::texture::GPUTexture,
) -> Result<String, JsErrorBox> {
  use deno_runtime::deno_canvas::image::ExtendedColorType;
  use deno_runtime::deno_canvas::image::ImageEncoder;
  use deno_runtime::deno_webgpu::error::GPUError;
  use deno_runtime::deno_webgpu::*;
  use texture::GPUTextureFormat;

  // We only support the 8 bit per pixel formats with 4 channels
  // as such a pixel has 4 bytes
  const BYTES_PER_PIXEL: u32 = 4;

  let unpadded_bytes_per_row = texture.size.width * BYTES_PER_PIXEL;
  let padded_bytes_per_row_padding = (wgpu_types::COPY_BYTES_PER_ROW_ALIGNMENT
    - (unpadded_bytes_per_row % wgpu_types::COPY_BYTES_PER_ROW_ALIGNMENT))
    % wgpu_types::COPY_BYTES_PER_ROW_ALIGNMENT;
  let padded_bytes_per_row =
    unpadded_bytes_per_row + padded_bytes_per_row_padding;

  let (buffer, maybe_err) = texture.instance.device_create_buffer(
    texture.device_id,
    &wgpu_types::BufferDescriptor {
      label: None,
      size: (padded_bytes_per_row * texture.size.height) as _,
      usage: wgpu_types::BufferUsages::MAP_READ
        | wgpu_types::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    },
    None,
  );
  if let Some(maybe_err) = maybe_err {
    return Err(JsErrorBox::from_err::<GPUError>(maybe_err.into()));
  }

  let (command_encoder, maybe_err) =
    texture.instance.device_create_command_encoder(
      texture.device_id,
      &wgpu_types::CommandEncoderDescriptor { label: None },
      None,
    );
  if let Some(maybe_err) = maybe_err {
    return Err(JsErrorBox::from_err::<GPUError>(maybe_err.into()));
  }

  texture
    .instance
    .command_encoder_copy_texture_to_buffer(
      command_encoder,
      &wgpu_types::TexelCopyTextureInfo {
        texture: texture.id,
        mip_level: 0,
        origin: Default::default(),
        aspect: Default::default(),
      },
      &wgpu_types::TexelCopyBufferInfo {
        buffer,
        layout: wgpu_types::TexelCopyBufferLayout {
          offset: 0,
          bytes_per_row: Some(padded_bytes_per_row),
          rows_per_image: None,
        },
      },
      &texture.size,
    )
    .map_err(|e| JsErrorBox::from_err::<GPUError>(e.into()))?;

  let (command_buffer, maybe_err) = texture.instance.command_encoder_finish(
    command_encoder,
    &wgpu_types::CommandBufferDescriptor { label: None },
  );
  if let Some(maybe_err) = maybe_err {
    return Err(JsErrorBox::from_err::<GPUError>(maybe_err.into()));
  }

  let maybe_err = texture
    .instance
    .queue_submit(texture.queue_id, &[command_buffer])
    .err();
  if let Some((_, maybe_err)) = maybe_err {
    return Err(JsErrorBox::from_err::<GPUError>(maybe_err.into()));
  }

  let index = texture
    .instance
    .buffer_map_async(
      buffer,
      0,
      None,
      wgpu_core::resource::BufferMapOperation {
        host: wgpu_core::device::HostMap::Read,
        callback: None,
      },
    )
    .map_err(|e| JsErrorBox::from_err::<GPUError>(e.into()))?;

  texture
    .instance
    .device_poll(
      texture.device_id,
      wgpu_types::Maintain::WaitForSubmissionIndex(index),
    )
    .map_err(|e| JsErrorBox::from_err::<GPUError>(e.into()))?;

  let (slice_pointer, range_size) = texture
    .instance
    .buffer_get_mapped_range(buffer, 0, None)
    .map_err(|e| JsErrorBox::from_err::<GPUError>(e.into()))?;

  let data = {
    // SAFETY: creating a slice from pointer and length provided by wgpu and
    // then dropping it before unmapping
    let slice = unsafe {
      std::slice::from_raw_parts(slice_pointer.as_ptr(), range_size as usize)
    };

    let mut unpadded =
      Vec::with_capacity((unpadded_bytes_per_row * texture.size.height) as _);

    for i in 0..texture.size.height {
      unpadded.extend_from_slice(
        &slice[((i * padded_bytes_per_row) as usize)
          ..(((i + 1) * padded_bytes_per_row) as usize)]
          [..(unpadded_bytes_per_row as usize)],
      );
    }

    unpadded
  };

  let color_type = match texture.format {
    GPUTextureFormat::Rgba8unorm => ExtendedColorType::Rgba8,
    GPUTextureFormat::Rgba8unormSrgb => ExtendedColorType::Rgba8,
    GPUTextureFormat::Rgba8snorm => ExtendedColorType::Rgba8,
    GPUTextureFormat::Rgba8uint => ExtendedColorType::Rgba8,
    GPUTextureFormat::Rgba8sint => ExtendedColorType::Rgba8,
    GPUTextureFormat::Bgra8unorm => ExtendedColorType::Bgra8,
    GPUTextureFormat::Bgra8unormSrgb => ExtendedColorType::Bgra8,
    _ => {
      return Err(JsErrorBox::type_error(format!(
        "Unsupported texture format '{}'",
        texture.format.as_str()
      )))
    }
  };

  let mut out: Vec<u8> = vec![];

  let img =
    deno_runtime::deno_canvas::image::codecs::png::PngEncoder::new(&mut out);
  img
    .write_image(&data, texture.size.width, texture.size.height, color_type)
    .map_err(|e| JsErrorBox::type_error(e.to_string()))?;

  texture
    .instance
    .buffer_unmap(buffer)
    .map_err(|e| JsErrorBox::from_err::<GPUError>(e.into()))?;
  texture.instance.buffer_drop(buffer);

  Ok(deno_runtime::deno_web::forgiving_base64_encode(&out))
}

#[op2]
#[serde]
pub fn op_jupyter_get_buffer(
  #[cppgc] buffer: &deno_runtime::deno_webgpu::buffer::GPUBuffer,
) -> Result<Vec<u8>, deno_runtime::deno_webgpu::error::GPUError> {
  use deno_runtime::deno_webgpu::*;
  let index = buffer.instance.buffer_map_async(
    buffer.id,
    0,
    None,
    wgpu_core::resource::BufferMapOperation {
      host: wgpu_core::device::HostMap::Read,
      callback: None,
    },
  )?;

  buffer.instance.device_poll(
    buffer.device,
    wgpu_types::Maintain::WaitForSubmissionIndex(index),
  )?;

  let (slice_pointer, range_size) = buffer
    .instance
    .buffer_get_mapped_range(buffer.id, 0, None)?;

  let data = {
    // SAFETY: creating a slice from pointer and length provided by wgpu and
    // then dropping it before unmapping
    let slice = unsafe {
      std::slice::from_raw_parts(slice_pointer.as_ptr(), range_size as usize)
    };

    slice.to_vec()
  };

  buffer.instance.buffer_unmap(buffer.id)?;

  Ok(data)
}
