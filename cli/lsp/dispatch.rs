// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::state::ServerState;
use super::state::ServerStateSnapshot;
use super::state::Task;
use super::utils::from_json;
use super::utils::is_canceled;
use super::ServerError;

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use lsp_server::ErrorCode;
use lsp_server::Notification;
use lsp_server::Request;
use lsp_server::RequestId;
use lsp_server::Response;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt;
use std::panic;

pub struct NotificationDispatcher<'a> {
  pub notification: Option<Notification>,
  pub server_state: &'a mut ServerState,
}

impl<'a> NotificationDispatcher<'a> {
  pub fn on<N>(
    &mut self,
    f: fn(&mut ServerState, N::Params) -> Result<(), AnyError>,
  ) -> Result<&mut Self, AnyError>
  where
    N: lsp_types::notification::Notification + 'static,
    N::Params: DeserializeOwned + Send + 'static,
  {
    let notification = match self.notification.take() {
      Some(it) => it,
      None => return Ok(self),
    };
    let params = match notification.extract::<N::Params>(N::METHOD) {
      Ok(it) => it,
      Err(notification) => {
        self.notification = Some(notification);
        return Ok(self);
      }
    };
    f(self.server_state, params)?;
    Ok(self)
  }

  pub fn finish(&mut self) {
    if let Some(notification) = &self.notification {
      if !notification.method.starts_with("$/") {
        error!("unhandled notification: {:?}", notification);
      }
    }
  }
}

fn result_to_response<R>(
  id: RequestId,
  result: Result<R::Result, AnyError>,
) -> Response
where
  R: lsp_types::request::Request + 'static,
  R::Params: DeserializeOwned + 'static,
  R::Result: Serialize + 'static,
{
  match result {
    Ok(response) => Response::new_ok(id, &response),
    Err(err) => match err.downcast::<ServerError>() {
      Ok(server_error) => {
        Response::new_err(id, server_error.code, server_error.message)
      }
      Err(err) => {
        if is_canceled(&*err) {
          Response::new_err(
            id,
            ErrorCode::ContentModified as i32,
            "content modified".to_string(),
          )
        } else {
          Response::new_err(
            id,
            ErrorCode::InternalError as i32,
            err.to_string(),
          )
        }
      }
    },
  }
}

pub struct RequestDispatcher<'a> {
  pub request: Option<Request>,
  pub server_state: &'a mut ServerState,
}

impl<'a> RequestDispatcher<'a> {
  pub fn finish(&mut self) {
    if let Some(request) = self.request.take() {
      error!("unknown request: {:?}", request);
      let response = Response::new_err(
        request.id,
        ErrorCode::MethodNotFound as i32,
        "unknown request".to_string(),
      );
      self.server_state.respond(response);
    }
  }

  /// Handle a request which will respond to the LSP client asynchronously via
  /// a spawned thread.
  pub fn on<R>(
    &mut self,
    f: fn(ServerStateSnapshot, R::Params) -> Result<R::Result, AnyError>,
  ) -> &mut Self
  where
    R: lsp_types::request::Request + 'static,
    R::Params: DeserializeOwned + Send + fmt::Debug + 'static,
    R::Result: Serialize + 'static,
  {
    let (id, params) = match self.parse::<R>() {
      Some(it) => it,
      None => return self,
    };
    self.server_state.tasks.handle.spawn({
      let state = self.server_state.snapshot();
      move || {
        let result = f(state, params);
        Task::Response(result_to_response::<R>(id, result))
      }
    });

    self
  }

  /// Handle a request which will respond synchronously, returning a result if
  /// the request cannot be handled or has issues.
  pub fn on_sync<R>(
    &mut self,
    f: fn(&mut ServerState, R::Params) -> Result<R::Result, AnyError>,
  ) -> Result<&mut Self, AnyError>
  where
    R: lsp_types::request::Request + 'static,
    R::Params: DeserializeOwned + panic::UnwindSafe + fmt::Debug + 'static,
    R::Result: Serialize + 'static,
  {
    let (id, params) = match self.parse::<R>() {
      Some(it) => it,
      None => return Ok(self),
    };
    let state = panic::AssertUnwindSafe(&mut *self.server_state);

    let response = panic::catch_unwind(move || {
      let result = f(state.0, params);
      result_to_response::<R>(id, result)
    })
    .map_err(|_err| {
      custom_error(
        "SyncTaskPanic",
        format!("sync task {:?} panicked", R::METHOD),
      )
    })?;
    self.server_state.respond(response);
    Ok(self)
  }

  fn parse<R>(&mut self) -> Option<(RequestId, R::Params)>
  where
    R: lsp_types::request::Request + 'static,
    R::Params: DeserializeOwned + 'static,
  {
    let request = match &self.request {
      Some(request) if request.method == R::METHOD => {
        self.request.take().unwrap()
      }
      _ => return None,
    };

    let response = from_json(R::METHOD, request.params);
    match response {
      Ok(params) => Some((request.id, params)),
      Err(err) => {
        let response = Response::new_err(
          request.id,
          ErrorCode::InvalidParams as i32,
          err.to_string(),
        );
        self.server_state.respond(response);
        None
      }
    }
  }
}
