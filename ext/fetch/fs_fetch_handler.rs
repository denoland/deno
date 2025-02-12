// Copyright 2018-2025 the Deno authors. MIT license.

use std::rc::Rc;

use deno_core::futures::FutureExt;
use deno_core::futures::TryFutureExt;
use deno_core::futures::TryStreamExt;
use deno_core::url::Url;
use deno_core::CancelFuture;
use deno_core::OpState;
use deno_error::JsErrorBox;
use deno_fs::open_options_with_access_check;
use deno_fs::OpenOptions;
use deno_permissions::PermissionsContainer;
use http::StatusCode;
use http_body_util::BodyExt;
use tokio_util::io::ReaderStream;

use crate::CancelHandle;
use crate::CancelableResponseFuture;
use crate::FetchHandler;
use crate::FetchPermissions;

fn sync_permission_check<'a, P: FetchPermissions + 'static>(
  permissions: &'a mut P,
  api_name: &'static str,
) -> impl deno_fs::AccessCheckFn + 'a {
  move |resolved, path, _options| {
    permissions.check_read(resolved, path, api_name)
  }
}

/// An implementation which tries to read file URLs from the file system via
/// tokio::fs.
#[derive(Clone)]
pub struct FsFetchHandler;

impl FetchHandler for FsFetchHandler {
  fn fetch_file(
    &self,
    state: &mut OpState,
    url: &Url,
  ) -> (CancelableResponseFuture, Option<Rc<CancelHandle>>) {
    let mut access_check = sync_permission_check::<PermissionsContainer>(
      state.borrow_mut(),
      "fetch()",
    );
    let cancel_handle = CancelHandle::new_rc();
    let path = match url.to_file_path() {
      Ok(path) => path,
      Err(_) => {
        let fut = async move { Err::<_, _>(()) };
        return (
          fut
            .map_err(move |_| super::FetchError::NetworkError)
            .or_cancel(&cancel_handle)
            .boxed_local(),
          Some(cancel_handle),
        );
      }
    };
    let path_and_opts_result = open_options_with_access_check(
      OpenOptions {
        read: true,
        ..Default::default()
      },
      &path,
      Some(&mut access_check),
    );
    let response_fut = async move {
      let (path, opts) = path_and_opts_result?;
      let file = tokio::fs::OpenOptions::from(opts)
        .open(path)
        .await
        .map_err(|_| super::FetchError::NetworkError)?;
      let stream = ReaderStream::new(file)
        .map_ok(hyper::body::Frame::data)
        .map_err(JsErrorBox::from_err);

      let body = http_body_util::StreamBody::new(stream).boxed();
      let response = http::Response::builder()
        .status(StatusCode::OK)
        .body(body)
        .map_err(move |_| super::FetchError::NetworkError)?;
      Ok::<_, _>(response)
    }
    .or_cancel(&cancel_handle)
    .boxed_local();

    (response_fut, Some(cancel_handle))
  }
}
