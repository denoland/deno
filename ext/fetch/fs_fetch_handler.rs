// Copyright 2018-2025 the Deno authors. MIT license.

use std::rc::Rc;

use deno_core::futures::FutureExt;
use deno_core::futures::TryFutureExt;
use deno_core::futures::TryStreamExt;
use deno_core::url::Url;
use deno_core::CancelFuture;
use deno_core::OpState;
use deno_error::JsErrorBox;
use http::StatusCode;
use http_body_util::BodyExt;
use tokio_util::io::ReaderStream;

use crate::CancelHandle;
use crate::CancelableResponseFuture;
use crate::FetchHandler;

/// An implementation which tries to read file URLs from the file system via
/// tokio::fs.
#[derive(Clone)]
pub struct FsFetchHandler;

impl FetchHandler for FsFetchHandler {
  fn fetch_file(
    &self,
    _state: &mut OpState,
    url: &Url,
  ) -> (CancelableResponseFuture, Option<Rc<CancelHandle>>) {
    let cancel_handle = CancelHandle::new_rc();
    let path_result = url.to_file_path();
    let response_fut = async move {
      let path = path_result?;
      let file = tokio::fs::File::open(path).map_err(|_| ()).await?;
      let stream = ReaderStream::new(file)
        .map_ok(hyper::body::Frame::data)
        .map_err(JsErrorBox::from_err);
      let body = http_body_util::StreamBody::new(stream).boxed();
      let response = http::Response::builder()
        .status(StatusCode::OK)
        .body(body)
        .map_err(|_| ())?;
      Ok::<_, ()>(response)
    }
    .map_err(move |_| super::FetchError::NetworkError)
    .or_cancel(&cancel_handle)
    .boxed_local();

    (response_fut, Some(cancel_handle))
  }
}
