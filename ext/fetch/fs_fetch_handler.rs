// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::CancelHandle;
use crate::CancelableResponseFuture;
use crate::FetchHandler;
use crate::FetchRequestBodyResource;

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::url::Url;
use std::rc::Rc;

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
    let path = url.to_file_path().unwrap();
    let response_fut = async move {
      let response = {
        match tokio::fs::read(&path).await {
          Ok(body) => match http::Response::builder()
            .status(http::StatusCode::OK)
            .body(reqwest::Body::from(body))
          {
            Ok(response) => Ok(reqwest::Response::from(response)),
            Err(err) => Err(err.into()),
          },
          Err(err) => Err(err.into()),
        }
      };
      Ok(response)
    }
    .boxed_local();

    (response_fut, None, None)
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
