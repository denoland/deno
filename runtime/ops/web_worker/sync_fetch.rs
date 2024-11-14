// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use crate::web_worker::WebWorkerInternalHandle;
use crate::web_worker::WebWorkerType;
use deno_core::futures::StreamExt;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::OpState;
use deno_fetch::data_url::DataUrl;
use deno_fetch::FetchError;
use deno_web::BlobStore;
use http_body_util::BodyExt;
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

#[derive(Debug, thiserror::Error)]
pub enum SyncFetchError {
  #[error("Blob URLs are not supported in this context.")]
  BlobUrlsNotSupportedInContext,
  #[error("{0}")]
  Io(#[from] std::io::Error),
  #[error("Invalid script URL")]
  InvalidScriptUrl,
  #[error("http status error: {0}")]
  InvalidStatusCode(http::StatusCode),
  #[error("Classic scripts with scheme {0}: are not supported in workers")]
  ClassicScriptSchemeUnsupportedInWorkers(String),
  #[error("{0}")]
  InvalidUri(#[from] http::uri::InvalidUri),
  #[error("Invalid MIME type {0:?}.")]
  InvalidMimeType(String),
  #[error("Missing MIME type.")]
  MissingMimeType,
  #[error(transparent)]
  Fetch(#[from] FetchError),
  #[error(transparent)]
  Join(#[from] tokio::task::JoinError),
  #[error(transparent)]
  Other(deno_core::error::AnyError),
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
) -> Result<Vec<SyncFetchScript>, SyncFetchError> {
  let handle = state.borrow::<WebWorkerInternalHandle>().clone();
  assert_eq!(handle.worker_type, WebWorkerType::Classic);

  // it's not safe to share a client across tokio runtimes, so create a fresh one
  // https://github.com/seanmonstar/reqwest/issues/1148#issuecomment-910868788
  let options = state.borrow::<deno_fetch::Options>().clone();
  let client = deno_fetch::create_client_from_options(&options)
    .map_err(FetchError::ClientCreate)?;

  // TODO(andreubotella) It's not good to throw an exception related to blob
  // URLs when none of the script URLs use the blob scheme.
  // Also, in which contexts are blob URLs not supported?
  let blob_store = state
    .try_borrow::<Arc<BlobStore>>()
    .ok_or(SyncFetchError::BlobUrlsNotSupportedInContext)?
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
              .map_err(|_| SyncFetchError::InvalidScriptUrl)?;
            let mut loose_mime_checks = loose_mime_checks;

            let (body, mime_type, res_url) = match script_url.scheme() {
              "http" | "https" => {
                let mut req = http::Request::new(
                  http_body_util::Empty::new()
                    .map_err(|never| match never {})
                    .boxed(),
                );
                *req.uri_mut() = script_url.as_str().parse()?;

                let resp =
                  client.send(req).await.map_err(FetchError::ClientSend)?;

                if resp.status().is_client_error()
                  || resp.status().is_server_error()
                {
                  return Err(SyncFetchError::InvalidStatusCode(resp.status()));
                }

                // TODO(andreubotella) Properly run fetch's "extract a MIME type".
                let mime_type = resp
                  .headers()
                  .get("Content-Type")
                  .and_then(|v| v.to_str().ok())
                  .map(mime_type_essence);

                // Always check the MIME type with HTTP(S).
                loose_mime_checks = false;

                let body = resp
                  .collect()
                  .await
                  .map_err(SyncFetchError::Other)?
                  .to_bytes();

                (body, mime_type, script)
              }
              "data" => {
                let data_url =
                  DataUrl::process(&script).map_err(FetchError::DataUrl)?;

                let mime_type = {
                  let mime = data_url.mime_type();
                  format!("{}/{}", mime.type_, mime.subtype)
                };

                let (body, _) =
                  data_url.decode_to_vec().map_err(FetchError::Base64)?;

                (Bytes::from(body), Some(mime_type), script)
              }
              "blob" => {
                let blob = blob_store
                  .get_object_url(script_url)
                  .ok_or(FetchError::BlobNotFound)?;

                let mime_type = mime_type_essence(&blob.media_type);

                let body = blob.read_all().await;

                (Bytes::from(body), Some(mime_type), script)
              }
              _ => {
                return Err(
                  SyncFetchError::ClassicScriptSchemeUnsupportedInWorkers(
                    script_url.scheme().to_string(),
                  ),
                )
              }
            };

            if !loose_mime_checks {
              // TODO(andreubotella) Check properly for a Javascript MIME type.
              match mime_type.as_deref() {
                Some("application/javascript" | "text/javascript") => {}
                Some(mime_type) => {
                  return Err(SyncFetchError::InvalidMimeType(
                    mime_type.to_string(),
                  ))
                }
                None => return Err(SyncFetchError::MissingMimeType),
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
