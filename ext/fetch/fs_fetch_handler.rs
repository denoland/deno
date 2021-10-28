// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::CancelHandle;
use crate::CancelableResponseFuture;
use crate::FetchHandler;
use crate::FetchRequestBodyResource;

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::url::Url;
use deno_core::CancelFuture;
use reqwest::StatusCode;
use std::io;
use std::rc::Rc;
use tokio_util::io::ReaderStream;

/// An implementation which tries to read file URLs from the file system via
/// tokio::fs
#[derive(Clone)]
pub struct FsFetchHandler;

impl FetchHandler for FsFetchHandler {
  fn fetch_url(
    &mut self,
    url: Url,
  ) -> (
    CancelableResponseFuture,
    Option<FetchRequestBodyResource>,
    Option<Rc<CancelHandle>>,
  ) {
    let cancel_handle = CancelHandle::new_rc();

    let response_fut = async move {
      let path = url
        .to_file_path()
        .map_err(|()| io::Error::from(io::ErrorKind::NotFound))?;
      let file = tokio::fs::File::open(path).await?;
      let stream = ReaderStream::new(file);
      let body = reqwest::Body::wrap_stream(stream);
      let response = http::Response::builder()
        .status(StatusCode::OK)
        .body(body)?
        .into();
      Ok::<_, AnyError>(response)
    }
    .or_cancel(&cancel_handle)
    .boxed_local();

    (response_fut, None, Some(cancel_handle))
  }

  fn validate_url(&mut self, url: &Url) -> Result<(), AnyError> {
    // Error messages are kept intentionally generic in order to discourage
    // probing, and attempting to discern something from the environment.
    let path = url
      .to_file_path()
      .map_err(|_| type_error(format!("Unable to fetch \"{}\".", url)))?;
    if !path.is_file() {
      Err(type_error(format!("Unable to fetch \"{}\".", url)))
    } else {
      Ok(())
    }
  }
}
