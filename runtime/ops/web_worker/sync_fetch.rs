// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::web_worker::WebWorkerInternalHandle;
use crate::web_worker::WebWorkerType;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::OpState;
use deno_fetch::reqwest;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFetchScript {
  url: String,
  script: String,
}

pub fn op_worker_sync_fetch(
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
