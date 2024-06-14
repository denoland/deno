// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use crate::web_worker::WebWorkerInternalHandle;
use crate::web_worker::WebWorkerType;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::OpState;
use deno_fetch::data_url::DataUrl;
use deno_web::BlobStore;
use deno_websocket::DomExceptionNetworkError;
use hyper::body::Bytes;
use serde::Deserialize;
use serde::Serialize;

// TODO(andreubotella) Properly parse the MIME type
fn mime_type_essence(mime_type: &str) -> String {
  let essence = match mime_type.split_once(';') {
    Some((essence, _)) => essence,
    None => mime_type,
  };
  essence.trim().to_ascii_lowercase()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFetchScript {
  url: String,
  script: String,
}

#[op2]
#[serde]
pub fn op_worker_sync_fetch(
  state: &mut OpState,
  #[serde] scripts: Vec<String>,
  loose_mime_checks: bool,
) -> Result<Vec<SyncFetchScript>, AnyError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  assert_eq!(handle.worker_type, WebWorkerType::Classic);

  // it's not safe to share a client across tokio runtimes, so create a fresh one
  // https://github.com/seanmonstar/reqwest/issues/1148#issuecomment-910868788
  let options = state.borrow::<deno_fetch::Options>().clone();
  let client = deno_fetch::create_client_from_options(&options)?;

  // TODO(andreubotella) It's not good to throw an exception related to blob
  // URLs when none of the script URLs use the blob scheme.
  // Also, in which contexts are blob URLs not supported?
  let blob_store = state
    .try_borrow::<Arc<BlobStore>>()
    .ok_or_else(|| type_error("Blob URLs are not supported in this context."))?
    .clone();

  // TODO(andreubotella): make the below thread into a resource that can be
  // re-used. This would allow parallel fetching of multiple scripts.

  let thread = std::thread::spawn(move || {
    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_io()
      .enable_time()
      .build()?;

    runtime.block_on(async move {
      let mut futures = scripts
        .into_iter()
        .map(|script| {
          let client = client.clone();
          let blob_store = blob_store.clone();
          deno_core::unsync::spawn(async move {
            let script_url = Url::parse(&script)
              .map_err(|_| type_error("Invalid script URL"))?;
            let mut loose_mime_checks = loose_mime_checks;

            let (body, mime_type, res_url) = match script_url.scheme() {
              "http" | "https" => {
                let resp =
                  client.get(script_url).send().await?.error_for_status()?;

                let res_url = resp.url().to_string();

                // TODO(andreubotella) Properly run fetch's "extract a MIME type".
                let mime_type = resp
                  .headers()
                  .get("Content-Type")
                  .and_then(|v| v.to_str().ok())
                  .map(mime_type_essence);

                // Always check the MIME type with HTTP(S).
                loose_mime_checks = false;

                let body = resp.bytes().await?;

                (body, mime_type, res_url)
              }
              "data" => {
                let data_url = DataUrl::process(&script)
                  .map_err(|e| type_error(format!("{e:?}")))?;

                let mime_type = {
                  let mime = data_url.mime_type();
                  format!("{}/{}", mime.type_, mime.subtype)
                };

                let (body, _) = data_url
                  .decode_to_vec()
                  .map_err(|e| type_error(format!("{e:?}")))?;

                (Bytes::from(body), Some(mime_type), script)
              }
              "blob" => {
                let blob =
                  blob_store.get_object_url(script_url).ok_or_else(|| {
                    type_error("Blob for the given URL not found.")
                  })?;

                let mime_type = mime_type_essence(&blob.media_type);

                let body = blob.read_all().await?;

                (Bytes::from(body), Some(mime_type), script)
              }
              _ => {
                return Err(type_error(format!(
              "Classic scripts with scheme {}: are not supported in workers.",
              script_url.scheme()
            )))
              }
            };

            if !loose_mime_checks {
              // TODO(andreubotella) Check properly for a Javascript MIME type.
              match mime_type.as_deref() {
                Some("application/javascript" | "text/javascript") => {}
                Some(mime_type) => {
                  return Err(
                    DomExceptionNetworkError {
                      msg: format!("Invalid MIME type {mime_type:?}."),
                    }
                    .into(),
                  )
                }
                None => {
                  return Err(
                    DomExceptionNetworkError::new("Missing MIME type.").into(),
                  )
                }
              }
            }

            let (text, _) = encoding_rs::UTF_8.decode_with_bom_removal(&body);

            Ok(SyncFetchScript {
              url: res_url,
              script: text.into_owned(),
            })
          })
        })
        .collect::<deno_core::futures::stream::FuturesUnordered<_>>();
      let mut ret = Vec::with_capacity(futures.len());
      while let Some(result) = futures.next().await {
        let script = result??;
        ret.push(script);
      }
      Ok(ret)
    })
  });

  thread.join().unwrap()
}
