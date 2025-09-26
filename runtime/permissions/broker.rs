// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU32;

use parking_lot::Mutex;

use super::BrokerResponse;

// TODO(bartlomieju): currently randomly selected exit code, it should
// be documented
static BROKER_EXIT_CODE: i32 = 87;

static PERMISSION_BROKER: OnceLock<PermissionBroker> = OnceLock::new();

pub fn set_broker(broker: PermissionBroker) {
  assert!(PERMISSION_BROKER.set(broker).is_ok());
}

pub fn has_broker() -> bool {
  PERMISSION_BROKER.get().is_some()
}

#[derive(serde::Serialize)]
struct PermissionBrokerRequest {
  v: u32,
  id: u32,
  datetime: String,
  permission: String,
  value: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PermissionBrokerResponse {
  id: u32,
  result: String,
}

pub struct PermissionBroker {
  stream: Mutex<UnixStream>,
  next_id: AtomicU32,
}

impl PermissionBroker {
  pub fn new(socket_path: impl Into<PathBuf>) -> Self {
    let stream = match UnixStream::connect(socket_path.into()) {
      Ok(s) => s,
      Err(err) => {
        log::error!("Failed to create permission broker: {:?}", err);
        std::process::exit(BROKER_EXIT_CODE);
      }
    };
    Self {
      stream: Mutex::new(stream),
      next_id: AtomicU32::new(1),
    }
  }

  fn check(
    &self,
    permission: &str,
    stringified_value: Option<String>,
  ) -> std::io::Result<BrokerResponse> {
    let mut stream = self.stream.lock();
    let id = self
      .next_id
      .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let request = PermissionBrokerRequest {
      v: 1,
      id,
      datetime: chrono::Utc::now().to_rfc3339(),
      permission: permission.to_string(),
      value: stringified_value,
    };

    let msg = format!("{}\n", serde_json::to_string(&request).unwrap());
    log::trace!("-> broker req   {}", msg);
    stream.write_all(msg.as_bytes())?;

    // Read response using line reader
    let mut reader = BufReader::new(&mut *stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;

    let response =
      serde_json::from_str::<PermissionBrokerResponse>(response_line.trim())
        .map_err(std::io::Error::other)?;

    log::trace!("<- broker resp  {:?}", response);

    if response.id != id {
      return Err(std::io::Error::other(
        "Permission broker response ID mismatch",
      ));
    }

    let prompt_response = match response.result.as_str() {
      "allow" => BrokerResponse::Allow,
      "deny" => BrokerResponse::Deny,
      _ => {
        return Err(std::io::Error::other(
          "Permission broker unknown result variant",
        ));
      }
    };

    Ok(prompt_response)
  }
}

pub fn maybe_check_with_broker(
  name: &str,
  stringified_value_fn: impl Fn() -> Option<String>,
) -> Option<BrokerResponse> {
  let broker = PERMISSION_BROKER.get()?;

  let resp = match broker.check(name, stringified_value_fn()) {
    Ok(resp) => resp,
    Err(err) => {
      log::error!("{:?}", err);
      std::process::exit(BROKER_EXIT_CODE);
    }
  };
  Some(resp)
}
