// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::op2;
use deno_net::raw::NetworkListenerResource;
use deno_net::tcp::TcpListener;
use saffron::Cron;

pub const UNSTABLE_FEATURE_NAME: &str = "cron";

deno_core::extension!(
  deno_cron,
  deps = [deno_net],
  ops = [op_cron_compute_next_deadline, op_cron_get_net_handler,],
  esm = ["01_cron.ts"],
);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CronError {
  #[class(type)]
  #[error("Invalid cron schedule")]
  InvalidCron,
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(type)]
  #[error("Invalid sock address")]
  InvalidSockAddr,
}

#[op2(fast)]
fn op_cron_compute_next_deadline(
  #[string] schedule: &str,
) -> Result<f64, CronError> {
  let Ok(cron) = schedule.parse::<Cron>() else {
    return Err(CronError::InvalidCron);
  };

  let now = chrono::Utc::now();

  if let Ok(test_schedule) = std::env::var("DENO_CRON_TEST_SCHEDULE_OFFSET")
    && let Ok(offset) = test_schedule.parse::<u64>()
  {
    return Ok(
      (now + chrono::TimeDelta::milliseconds(offset as _)).timestamp_millis()
        as f64,
    );
  }

  let Some(next_deadline) = cron.next_after(now) else {
    return Err(CronError::InvalidCron);
  };
  Ok(next_deadline.timestamp_millis() as f64)
}

#[op2]
fn op_cron_get_net_handler(
  state: &mut OpState,
) -> Result<Option<ResourceId>, CronError> {
  let Ok(addr) = std::env::var("DENO_UNSTABLE_CRON_SOCK") else {
    return Ok(None);
  };

  let rid = match addr.split_once(":") {
    Some(("tcp", addr)) => {
      let resource = NetworkListenerResource::new(TcpListener::bind(
        addr.parse().map_err(|_| CronError::InvalidSockAddr)?,
        false,
        16,
      )?);
      state.resource_table.add(resource)
    }
    #[cfg(unix)]
    Some(("unix", path)) => {
      let listener = tokio::net::UnixListener::bind(path)?;
      let resource = NetworkListenerResource::new(
        deno_net::ops_unix::UnixListenerWithPath::new(listener, path.into()),
      );
      state.resource_table.add(resource)
    }
    #[cfg(any(
      target_os = "android",
      target_os = "linux",
      target_os = "macos"
    ))]
    Some(("vsock", addr)) => {
      let Some((cid, port)) = addr.split_once(":") else {
        return Err(
          std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid vsock addr",
          )
          .into(),
        );
      };
      let cid = if cid == "-1" {
        u32::MAX
      } else {
        cid.parse().map_err(|_| CronError::InvalidSockAddr)?
      };
      let port = port.parse().map_err(|_| CronError::InvalidSockAddr)?;
      let resource =
        NetworkListenerResource::new(tokio_vsock::VsockListener::bind(
          tokio_vsock::VsockAddr::new(cid, port),
        )?);
      state.resource_table.add(resource)
    }
    _ => return Ok(None),
  };

  Ok(Some(rid))
}
