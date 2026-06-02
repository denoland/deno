// Copyright 2018-2026 the Deno authors. MIT license.

mod handler_impl;
mod interface;
pub mod local;
mod socket;

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use deno_features::FeatureChecker;
pub use handler_impl::CronHandlerImpl;
pub use socket::SocketCronHandle;
pub use socket::SocketCronHandler;

pub use crate::interface::*;

pub const UNSTABLE_FEATURE_NAME: &str = "cron";

deno_core::extension!(deno_cron,
  ops = [
    op_cron_create,
    op_cron_next,
  ],
  lazy_loaded_js = [ "01_cron.ts" ],
  options = {
    cron_handler: Box<dyn CronHandler>,
  },
  state = |state, options| {
    state.put::<Rc<dyn CronHandler>>(Rc::from(options.cron_handler));
  }
);

struct CronResource {
  handle: Rc<dyn CronHandle>,
}

impl Resource for CronResource {
  fn name(&self) -> Cow<'_, str> {
    "cron".into()
  }

  fn close(self: Rc<Self>) {
    self.handle.close();
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CronError {
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(type)]
  #[error("Cron name cannot exceed 64 characters: current length {0}")]
  NameExceeded(usize),
  #[class(type)]
  #[error(
    "Invalid cron name: only alphanumeric characters, whitespace, hyphens, and underscores are allowed"
  )]
  NameInvalid,
  #[class(type)]
  #[error("Cron with this name already exists")]
  AlreadyExists,
  #[class(type)]
  #[error("Too many crons")]
  TooManyCrons,
  #[class(type)]
  #[error("Invalid cron schedule")]
  InvalidCron,
  #[class(type)]
  #[error("Invalid backoff schedule")]
  InvalidBackoff,
  #[class(generic)]
  #[error(transparent)]
  AcquireError(#[from] tokio::sync::AcquireError),
  #[class(generic)]
  #[error("Cron socket error: {0}")]
  SocketError(String),
  #[class(generic)]
  #[error("Error registering cron: {0}")]
  RejectedError(String),
  #[class(inherit)]
  #[error(transparent)]
  Other(JsErrorBox),
}

#[op2]
#[smi]
fn op_cron_create(
  state: Rc<RefCell<OpState>>,
  #[string] name: String,
  #[string] cron_schedule: String,
  #[scoped] backoff_schedule: Option<Vec<u32>>,
) -> Result<ResourceId, CronError> {
  let cron_handler = {
    let state = state.borrow();
    state
      .borrow::<Arc<FeatureChecker>>()
      .check_or_exit(UNSTABLE_FEATURE_NAME, "Deno.cron");
    state.borrow::<Rc<dyn CronHandler>>().clone()
  };

  validate_cron_name(&name)?;

  let handle = cron_handler.create(CronSpec {
    name,
    cron_schedule,
    backoff_schedule,
  })?;

  let handle_rid = {
    let mut state = state.borrow_mut();
    state.resource_table.add(CronResource { handle })
  };
  Ok(handle_rid)
}

#[op2]
#[serde]
async fn op_cron_next(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  prev_success: bool,
) -> Result<CronNextResult, CronError> {
  let cron_handle = {
    let state = state.borrow();
    let resource = match state.resource_table.get::<CronResource>(rid) {
      Ok(resource) => resource,
      Err(err) => {
        if err.get_class() == "BadResource" {
          return Ok(CronNextResult {
            active: false,
            traceparent: None,
          });
        } else {
          return Err(CronError::Resource(err));
        }
      }
    };
    resource.handle.clone()
  };

  cron_handle.next(prev_success).await
}

fn validate_cron_name(name: &str) -> Result<(), CronError> {
  if name.len() > 64 {
    return Err(CronError::NameExceeded(name.len()));
  }
  if !name.chars().all(|c| {
    c.is_ascii_whitespace() || c.is_ascii_alphanumeric() || c == '_' || c == '-'
  }) {
    return Err(CronError::NameInvalid);
  }
  Ok(())
}
