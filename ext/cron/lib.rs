// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::op2;
use deno_net::raw::NetworkStreamListener;
use deno_net::tcp::TcpListener;
use saffron::Cron;

pub const UNSTABLE_FEATURE_NAME: &str = "cron";

deno_core::extension!(
  deno_cron,
  deps = [deno_net],
  ops = [op_cron_compute_next_deadline, op_cron_take_net_handler],
  esm = ["01_cron.ts"],
  state = |state| {
    if let Ok(Some(handler)) = get_net_handler() {
      state.put(NetHandler(handler));
    }
  },
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

struct NetHandler(NetworkStreamListener);

fn get_net_handler() -> Result<Option<NetworkStreamListener>, CronError> {
  let Ok(addr) = std::env::var("DENO_UNSTABLE_CRON_SOCK") else {
    return Ok(None);
  };

  let listener = match addr.split_once(":") {
    Some(("tcp", addr)) => TcpListener::bind(
      addr.parse().map_err(|_| CronError::InvalidSockAddr)?,
      false,
      16,
    )?
    .into(),
    #[cfg(unix)]
    Some(("unix", path)) => {
      let listener = tokio::net::UnixListener::bind(path)?;
      deno_net::ops_unix::UnixListenerWithPath::new(listener, path.into())
        .into()
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
      tokio_vsock::VsockListener::bind(tokio_vsock::VsockAddr::new(cid, port))?
        .into()
    }
    _ => return Ok(None),
  };

  Ok(Some(listener))
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
fn op_cron_take_net_handler(
  state: &mut OpState,
) -> Result<Option<ResourceId>, CronError> {
  let Some(handler) = state.try_take::<NetHandler>() else {
    return Ok(None);
  };

  let rid = handler.0.into_resource(&mut state.resource_table);

  Ok(Some(rid))
}
