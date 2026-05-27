// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::OpState;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::serde_json;
use deno_error::JsErrorBox;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::cdp;

// ------------------------------------------------------------------
// Shared data types
// ------------------------------------------------------------------

/// An iopub message produced by the REPL thread to be published by the kernel.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IopubMessage {
  pub msg_type: String,
  pub content: serde_json::Value,
  pub metadata: serde_json::Value,
  pub buffers: Vec<Vec<u8>>,
}

// ------------------------------------------------------------------
// REPL request / response types (with embedded oneshot responders)
// ------------------------------------------------------------------

pub enum JupyterReplRequest {
  Evaluate {
    line: String,
    resp_tx: oneshot::Sender<Option<serde_json::Value>>,
  },
  GetProperties {
    object_id: String,
    resp_tx: oneshot::Sender<Option<serde_json::Value>>,
  },
  GlobalLexicalScopeNames {
    resp_tx: oneshot::Sender<serde_json::Value>,
  },
  CallFunctionOnArgs {
    function_declaration: String,
    args: Vec<cdp::RemoteObject>,
    resp_tx: oneshot::Sender<Result<serde_json::Value, AnyError>>,
  },
  CallFunctionOn {
    arg0: cdp::CallArgument,
    arg1: cdp::CallArgument,
    resp_tx: oneshot::Sender<Option<serde_json::Value>>,
  },
}

// ------------------------------------------------------------------
// State stored in op_state of the ZMQ kernel worker (main thread)
// ------------------------------------------------------------------

pub struct KernelReplSender {
  pub tx: mpsc::UnboundedSender<JupyterReplRequest>,
}

pub struct KernelIopubReceiver {
  pub rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<IopubMessage>>,
}

pub struct KernelIsolateHandle {
  pub handle: deno_core::v8::IsolateHandle,
}

pub struct KernelConnectionInfo {
  pub json: String,
}

// ------------------------------------------------------------------
// State stored in op_state of the REPL worker (background thread)
// ------------------------------------------------------------------

pub struct ReplIopubSender {
  pub tx: mpsc::UnboundedSender<IopubMessage>,
}

// ------------------------------------------------------------------
// Extension declarations
// ------------------------------------------------------------------

deno_core::extension!(
  deno_jupyter_kernel,
  ops = [
    op_jupyter_get_connection_info,
    op_jupyter_repl_evaluate,
    op_jupyter_repl_get_properties,
    op_jupyter_repl_global_lexical_scope_names,
    op_jupyter_repl_call_function_on_args,
    op_jupyter_repl_call_function_on,
    op_jupyter_repl_interrupt,
    op_jupyter_repl_cancel_interrupt,
    op_jupyter_recv_iopub,
    op_jupyter_deno_version,
    op_jupyter_typescript_version,
  ],
);

deno_core::extension!(
  deno_jupyter_repl,
  ops = [
    op_jupyter_broadcast,
    op_jupyter_create_png_from_texture,
    op_jupyter_get_buffer,
  ],
  options = {
    sender: mpsc::UnboundedSender<IopubMessage>,
  },
  middleware = |op| match op.name {
    "op_print" => op_print(),
    _ => op,
  },
  state = |state, options| {
    state.put(ReplIopubSender { tx: options.sender });
  },
);

// Variant used when running tests (no middleware so stdout/stderr pass through).
deno_core::extension!(
  deno_jupyter_repl_for_test,
  ops = [
    op_jupyter_broadcast,
    op_jupyter_create_png_from_texture,
    op_jupyter_get_buffer,
  ],
  options = {
    sender: mpsc::UnboundedSender<IopubMessage>,
  },
  state = |state, options| {
    state.put(ReplIopubSender { tx: options.sender });
  },
);

// Backward-compat alias used by cli/tools/test/mod.rs
pub use deno_jupyter_repl_for_test as deno_jupyter_for_test;

// ------------------------------------------------------------------
// Kernel-side ops
// ------------------------------------------------------------------

#[op2]
#[string]
pub fn op_jupyter_get_connection_info(state: &mut OpState) -> String {
  state.borrow::<KernelConnectionInfo>().json.clone()
}

#[op2]
#[serde]
pub async fn op_jupyter_repl_evaluate(
  state: Rc<RefCell<OpState>>,
  #[string] line: String,
) -> Result<Option<serde_json::Value>, JsErrorBox> {
  let (resp_tx, resp_rx) = oneshot::channel();
  {
    let s = state.borrow();
    let sender = s.borrow::<KernelReplSender>();
    sender
      .tx
      .send(JupyterReplRequest::Evaluate { line, resp_tx })
      .map_err(|_| JsErrorBox::generic("repl thread gone"))?;
  }
  resp_rx
    .await
    .map_err(|_| JsErrorBox::generic("repl response channel closed"))
}

#[op2]
#[serde]
pub async fn op_jupyter_repl_get_properties(
  state: Rc<RefCell<OpState>>,
  #[string] object_id: String,
) -> Result<Option<serde_json::Value>, JsErrorBox> {
  let (resp_tx, resp_rx) = oneshot::channel();
  {
    let s = state.borrow();
    s.borrow::<KernelReplSender>()
      .tx
      .send(JupyterReplRequest::GetProperties { object_id, resp_tx })
      .map_err(|_| JsErrorBox::generic("repl thread gone"))?;
  }
  resp_rx
    .await
    .map_err(|_| JsErrorBox::generic("repl response channel closed"))
}

#[op2]
#[serde]
pub async fn op_jupyter_repl_global_lexical_scope_names(
  state: Rc<RefCell<OpState>>,
) -> Result<serde_json::Value, JsErrorBox> {
  let (resp_tx, resp_rx) = oneshot::channel();
  {
    let s = state.borrow();
    s.borrow::<KernelReplSender>()
      .tx
      .send(JupyterReplRequest::GlobalLexicalScopeNames { resp_tx })
      .map_err(|_| JsErrorBox::generic("repl thread gone"))?;
  }
  resp_rx
    .await
    .map_err(|_| JsErrorBox::generic("repl response channel closed"))
}

#[op2]
#[serde]
pub async fn op_jupyter_repl_call_function_on_args(
  state: Rc<RefCell<OpState>>,
  #[string] function_declaration: String,
  #[serde] args: Vec<cdp::RemoteObject>,
) -> Result<serde_json::Value, JsErrorBox> {
  let (resp_tx, resp_rx) = oneshot::channel();
  {
    let s = state.borrow();
    s.borrow::<KernelReplSender>()
      .tx
      .send(JupyterReplRequest::CallFunctionOnArgs {
        function_declaration,
        args,
        resp_tx,
      })
      .map_err(|_| JsErrorBox::generic("repl thread gone"))?;
  }
  resp_rx
    .await
    .map_err(|_| JsErrorBox::generic("repl response channel closed"))?
    .map(Ok)
    .unwrap_or(Err(JsErrorBox::generic("call_function_on_args failed")))
}

#[op2]
#[serde]
pub async fn op_jupyter_repl_call_function_on(
  state: Rc<RefCell<OpState>>,
  #[serde] arg0: cdp::CallArgument,
  #[serde] arg1: cdp::CallArgument,
) -> Result<Option<serde_json::Value>, JsErrorBox> {
  let (resp_tx, resp_rx) = oneshot::channel();
  {
    let s = state.borrow();
    s.borrow::<KernelReplSender>()
      .tx
      .send(JupyterReplRequest::CallFunctionOn {
        arg0,
        arg1,
        resp_tx,
      })
      .map_err(|_| JsErrorBox::generic("repl thread gone"))?;
  }
  resp_rx
    .await
    .map_err(|_| JsErrorBox::generic("repl response channel closed"))
}

#[op2(fast)]
pub fn op_jupyter_repl_interrupt(state: &mut OpState) {
  state
    .borrow::<KernelIsolateHandle>()
    .handle
    .terminate_execution();
}

#[op2(fast)]
pub fn op_jupyter_repl_cancel_interrupt(state: &mut OpState) {
  state
    .borrow::<KernelIsolateHandle>()
    .handle
    .cancel_terminate_execution();
}

#[op2]
#[serde]
pub async fn op_jupyter_recv_iopub(
  state: Rc<RefCell<OpState>>,
) -> Result<Option<IopubMessage>, JsErrorBox> {
  let rx_ref = {
    let s = state.borrow();
    // SAFETY: We only call this from one JS async context at a time.
    let recv = s.borrow::<KernelIopubReceiver>();
    // We need a pointer trick since we can't hold the borrow across await.
    recv as *const KernelIopubReceiver as usize
  };
  // SAFETY: The KernelIopubReceiver lives as long as op_state.
  let rx = unsafe { &*(rx_ref as *const KernelIopubReceiver) };
  let mut guard = rx.rx.lock().await;
  Ok(guard.recv().await)
}

#[op2]
#[string]
pub fn op_jupyter_deno_version(_state: &mut OpState) -> String {
  deno_lib::version::DENO_VERSION_INFO.deno.to_string()
}

#[op2]
#[string]
pub fn op_jupyter_typescript_version(_state: &mut OpState) -> String {
  deno_lib::version::DENO_VERSION_INFO.typescript.to_string()
}

// ------------------------------------------------------------------
// REPL-side ops
// ------------------------------------------------------------------

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum JupyterBroadcastError {
  #[class(generic)]
  #[error(transparent)]
  Send(#[from] mpsc::error::SendError<IopubMessage>),
}

#[op2]
#[allow(
  clippy::result_large_err,
  reason = "IopubMessage is moved through the channel; boxing the error adds an allocation on the hot path."
)]
pub fn op_jupyter_broadcast(
  state: &mut OpState,
  #[string] message_type: String,
  #[serde] content: serde_json::Value,
  #[serde] metadata: serde_json::Value,
  #[serde] buffers: Vec<deno_core::JsBuffer>,
) -> Result<(), JupyterBroadcastError> {
  let sender = state.borrow::<ReplIopubSender>();
  sender.tx.send(IopubMessage {
    msg_type: message_type,
    content,
    metadata,
    buffers: buffers.into_iter().map(|b| b.to_vec()).collect(),
  })?;
  Ok(())
}

#[op2(fast)]
pub fn op_print(state: &mut OpState, #[string] msg: &str, is_err: bool) {
  let sender = state.borrow::<ReplIopubSender>();
  let msg_type = if is_err {
    "stream_stderr"
  } else {
    "stream_stdout"
  };
  let _ = sender.tx.send(IopubMessage {
    msg_type: msg_type.into(),
    content: serde_json::json!({ "name": if is_err { "stderr" } else { "stdout" }, "text": msg }),
    metadata: serde_json::Value::Object(Default::default()),
    buffers: vec![],
  });
}

#[op2]
#[string]
pub fn op_jupyter_create_png_from_texture(
  #[cppgc] texture: &deno_runtime::deno_webgpu::texture::GPUTexture,
) -> Result<String, JsErrorBox> {
  use deno_runtime::deno_image::image::ExtendedColorType;
  use deno_runtime::deno_image::image::ImageEncoder;
  use deno_runtime::deno_webgpu::error::GPUError;
  use deno_runtime::deno_webgpu::*;
  use texture::GPUTextureFormat;

  let (command_encoder, maybe_err) =
    texture.instance.device_create_command_encoder(
      texture.device_id,
      &wgpu_types::CommandEncoderDescriptor { label: None },
      None,
    );
  if let Some(maybe_err) = maybe_err {
    return Err(JsErrorBox::from_err::<GPUError>(maybe_err.into()));
  }

  let data = canvas::copy_texture_to_vec(
    &texture.instance,
    texture.device_id,
    texture.queue_id,
    command_encoder,
    texture.id,
    &texture.size,
  )?;

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
      )));
    }
  };

  let mut out: Vec<u8> = vec![];

  let img =
    deno_runtime::deno_image::image::codecs::png::PngEncoder::new(&mut out);
  img
    .write_image(&data, texture.size.width, texture.size.height, color_type)
    .map_err(|e| JsErrorBox::type_error(e.to_string()))?;

  Ok(deno_runtime::deno_web::forgiving_base64_encode(&out))
}

#[op2]
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

  buffer
    .instance
    .device_poll(
      buffer.device,
      wgpu_types::PollType::Wait {
        submission_index: Some(index),
        timeout: None,
      },
    )
    .unwrap();

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
