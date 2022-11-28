// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::CancelHandle;
use crate::CancelableResponseFuture;
use crate::FetchHandler;
use crate::FetchRequestBodyResource;

use deno_core::error::type_error;
use deno_core::futures::FutureExt;
use deno_core::futures::TryFutureExt;
use deno_core::url::Url;
use deno_core::CancelFuture;
use deno_core::OpState;
use reqwest::StatusCode;
use std::rc::Rc;
use tokio_util::io::ReaderStream;

/// An implementation which tries to read file URLs from the file system via
/// tokio::fs.
#[derive(Clone)]
pub struct FsFetchHandler;

impl FetchHandler for FsFetchHandler {
  fn fetch_file(
    &self,
    _state: &mut OpState,
    url: Url,
  ) -> (
    CancelableResponseFuture,
    Option<FetchRequestBodyResource>,
    Option<Rc<CancelHandle>>,
  ) {
    let cancel_handle = CancelHandle::new_rc();
    let response_fut = async move {
      let path = url.to_file_path()?;
      let file = tokio::fs::File::open(path).map_err(|_| ()).await?;
      let stream = ReaderStream::new(file);
      let body = reqwest::Body::wrap_stream(stream);
      let response = http::Response::builder()
        .status(StatusCode::OK)
        .body(body)
        .map_err(|_| ())?
        .into();
      Ok::<_, ()>(response)
    }
    .map_err(move |_| {
      type_error("NetworkError when attempting to fetch resource.")
    })
    .or_cancel(&cancel_handle)
    .boxed_local();

    (response_fut, None, Some(cancel_handle))
  }
}
