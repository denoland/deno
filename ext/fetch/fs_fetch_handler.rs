// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::rc::Rc;

use deno_core::CancelFuture;
use deno_core::OpState;
use deno_core::futures::FutureExt;
use deno_core::url::Url;
use deno_fs::FileSystemRc;
use deno_fs::OpenOptions;
use deno_io::fs::FileResource;
use deno_permissions::CheckedPath;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use http_body_util::combinators::BoxBody;

use crate::CancelHandle;
use crate::CancelableResponseFuture;
use crate::FetchHandler;
use crate::ResourceToBodyAdapter;

/// An implementation which tries to read file URLs via `deno_fs::FileSystem`.
#[derive(Clone)]
pub struct FsFetchHandler;

impl FetchHandler for FsFetchHandler {
  fn fetch_file(
    &self,
    state: &mut OpState,
    url: &Url,
  ) -> (CancelableResponseFuture, Option<Rc<CancelHandle>>) {
    let cancel_handle = CancelHandle::new_rc();
    let Ok(path) = url.to_file_path() else {
      return (
        async move { Err(super::FetchError::NetworkError) }
          .or_cancel(&cancel_handle)
          .boxed_local(),
        Some(cancel_handle),
      );
    };
    let fs = state.borrow::<FileSystemRc>().clone();
    let path = state
      .borrow::<PermissionsContainer>()
      .check_open(Cow::Owned(path), OpenAccessKind::Read, Some("fetch()"))
      .map(CheckedPath::into_owned);
    let response_fut = async move {
      let file = fs
        .open_async(path?, OpenOptions::read())
        .await
        .map_err(|_| super::FetchError::NetworkError)?;
      let resource = Rc::new(FileResource::new(file, "".to_owned()));
      let body = BoxBody::new(ResourceToBodyAdapter::new(resource));
      let response = http::Response::new(body);
      Ok(response)
    }
    .or_cancel(&cancel_handle)
    .boxed_local();

    (response_fut, Some(cancel_handle))
  }
}
