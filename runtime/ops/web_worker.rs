// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::web_worker::WebWorkerInternalHandle;
use crate::web_worker::WebWorkerType;
use crate::web_worker::WorkerControlEvent;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::CancelFuture;
use deno_core::Extension;
use deno_core::OpState;
use deno_fetch::reqwest;
use deno_web::JsMessageData;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::task::JoinHandle;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![
      ("op_worker_post_message", op_sync(op_worker_post_message)),
      ("op_worker_recv_message", op_async(op_worker_recv_message)),
      // Notify host that guest worker closes.
      ("op_worker_close", op_sync(op_worker_close)),
      // Notify host that guest worker has unhandled error.
      (
        "op_worker_unhandled_error",
        op_sync(op_worker_unhandled_error),
      ),
      ("op_worker_get_type", op_sync(op_worker_get_type)),
      ("op_worker_sync_fetch", op_sync(op_worker_sync_fetch)),
    ])
    .build()
}

fn op_worker_post_message(
  state: &mut OpState,
  data: JsMessageData,
  _: (),
) -> Result<(), AnyError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  handle.port.send(state, data)?;
  Ok(())
}

async fn op_worker_recv_message(
  state: Rc<RefCell<OpState>>,
  _: (),
  _: (),
) -> Result<Option<JsMessageData>, AnyError> {
  let handle = {
    let state = state.borrow();
    state.borrow::<WebWorkerInternalHandle>().clone()
  };
  handle
    .port
    .recv(state.clone())
    .or_cancel(handle.cancel)
    .await?
}

fn op_worker_close(state: &mut OpState, _: (), _: ()) -> Result<(), AnyError> {
  // Notify parent that we're finished
  let mut handle = state.borrow_mut::<WebWorkerInternalHandle>().clone();

  handle.terminate();
  Ok(())
}

/// A worker that encounters an uncaught error will pass this error
/// to its parent worker using this op. The parent worker will use
/// this same op to pass the error to its own parent (in case
/// `e.preventDefault()` was not called in `worker.onerror`). This
/// is done until the error reaches the root/ main worker.
fn op_worker_unhandled_error(
  state: &mut OpState,
  message: String,
  _: (),
) -> Result<(), AnyError> {
  let sender = state.borrow::<WebWorkerInternalHandle>().clone();
  sender
    .post_event(WorkerControlEvent::Error(generic_error(message)))
    .expect("Failed to propagate error event to parent worker");
  Ok(())
}

fn op_worker_get_type(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<WebWorkerType, AnyError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  Ok(handle.worker_type)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncFetchScript {
  url: String,
  script: String,
}

fn op_worker_sync_fetch(
  state: &mut OpState,
  scripts: Vec<String>,
  _: (),
) -> Result<Vec<SyncFetchScript>, AnyError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  assert_eq!(handle.worker_type, WebWorkerType::Classic);

  // TODO(andreubotella) Make the runtime into a resource and add a new op to
  // block on each request, so a script can run while the next loads.

  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    .build()
    .unwrap();

  let handles: Vec<_> = scripts
    .into_iter()
    .map(|script| -> JoinHandle<Result<SyncFetchScript, AnyError>> {
      runtime.spawn(async move {
        let resp = reqwest::get(script).await?.error_for_status()?;

        let url = resp.url().to_string();

        // TODO(andreubotella) Do a proper check that the MIME type is a
        // Javascript MIME type.
        let mime_type = resp
          .headers()
          .get("Content-Type")
          .and_then(|v| v.to_str().ok())
          .map(|v| {
            v.split_once(";")
              .unwrap_or((v, ""))
              .0
              .trim()
              .to_ascii_lowercase()
          });
        match mime_type.as_deref() {
          Some("application/javascript") => {}
          Some("text/javascript") => {}
          _ => {
            return Err(generic_error(format!(
              "Invalid MIME type {:?}.",
              mime_type
            )))
          }
        }

        // We don't use `resp.text()` or `resp.text_with_charset()` because
        // they will use the BOM or the MIME type's encoding.
        let body = resp.bytes().await?;
        let (text, _) = encoding_rs::UTF_8.decode_with_bom_removal(&body);

        Ok(SyncFetchScript {
          url,
          script: text.into_owned(),
        })
      })
    })
    .collect();

  let mut ret = Vec::with_capacity(handles.len());
  for handle in handles {
    let script = runtime.block_on(handle)??;
    ret.push(script);
  }
  Ok(ret)
}
