// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod interface;
pub mod local;

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

pub use crate::interface::*;
use deno_core::error::get_custom_error_class;
use deno_core::op2;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;

pub const UNSTABLE_FEATURE_NAME: &str = "cron";

deno_core::extension!(deno_cron,
  deps = [ deno_console ],
  parameters = [ C: CronHandler ],
  ops = [
    op_cron_create<C>,
    op_cron_next<C>,
  ],
  esm = [ "01_cron.ts" ],
  options = {
    cron_handler: C,
  },
  state = |state, options| {
    state.put(Rc::new(options.cron_handler));
  }
);

struct CronResource<EH: CronHandle + 'static> {
  handle: Rc<EH>,
}

impl<EH: CronHandle + 'static> Resource for CronResource<EH> {
  fn name(&self) -> Cow<str> {
    "cron".into()
  }

  fn close(self: Rc<Self>) {
    self.handle.close();
  }
}

#[derive(Debug, thiserror::Error)]
pub enum CronError {
  #[error(transparent)]
  Resource(deno_core::error::AnyError),
  #[error("Cron name cannot exceed 64 characters: current length {0}")]
  NameExceeded(usize),
  #[error("Invalid cron name: only alphanumeric characters, whitespace, hyphens, and underscores are allowed")]
  NameInvalid,
  #[error("Cron with this name already exists")]
  AlreadyExists,
  #[error("Too many crons")]
  TooManyCrons,
  #[error("Invalid cron schedule")]
  InvalidCron,
  #[error("Invalid backoff schedule")]
  InvalidBackoff,
  #[error(transparent)]
  AcquireError(#[from] tokio::sync::AcquireError),
  #[error(transparent)]
  Other(deno_core::error::AnyError),
}

#[op2]
#[smi]
fn op_cron_create<C>(
  state: Rc<RefCell<OpState>>,
  #[string] name: String,
  #[string] cron_schedule: String,
  #[serde] backoff_schedule: Option<Vec<u32>>,
) -> Result<ResourceId, CronError>
where
  C: CronHandler + 'static,
{
  let cron_handler = {
    let state = state.borrow();
    state
      .feature_checker
      .check_or_exit(UNSTABLE_FEATURE_NAME, "Deno.cron");
    state.borrow::<Rc<C>>().clone()
  };

  validate_cron_name(&name)?;

  let handle = cron_handler.create(CronSpec {
    name,
    cron_schedule,
    backoff_schedule,
  })?;

  let handle_rid = {
    let mut state = state.borrow_mut();
    state.resource_table.add(CronResource {
      handle: Rc::new(handle),
    })
  };
  Ok(handle_rid)
}

#[op2(async)]
async fn op_cron_next<C>(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  prev_success: bool,
) -> Result<bool, CronError>
where
  C: CronHandler + 'static,
{
  let cron_handler = {
    let state = state.borrow();
    let resource = match state.resource_table.get::<CronResource<C::EH>>(rid) {
      Ok(resource) => resource,
      Err(err) => {
        if get_custom_error_class(&err) == Some("BadResource") {
          return Ok(false);
        } else {
          return Err(CronError::Resource(err));
        }
      }
    };
    resource.handle.clone()
  };

  cron_handler.next(prev_success).await
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
