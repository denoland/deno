// Copyright 2018-2025 the Deno authors. MIT license.

// Partially extracted / adapted from https://github.com/microsoft/libsyncrpc
// Copyright 2024 Microsoft Corporation. MIT license.

pub mod connection;
pub mod types;

use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::BufReader;
use std::io::BufWriter;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;

use connection::RpcConnection;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("Failed to spawn process: {0}")]
  ProcessSpawn(#[source] std::io::Error),

  #[error("Failed to kill process: {0}")]
  ProcessKill(#[source] std::io::Error),

  #[error("Error in RPC connection: {0}")]
  RpcConnection(#[source] std::io::Error),

  #[error("Error encoding {obj} as {ty}: {source}")]
  Encoding {
    obj: &'static str,
    ty: &'static str,
    source: Box<Error>,
  },

  #[error("Error decoding UTF-8: {0}")]
  Utf8(#[source] std::string::FromUtf8Error),

  #[error("Invalid message type: {0}")]
  InvalidMessageType(MessageType),

  #[error("{0}")]
  AdHoc(String),

  #[error("serde json error: {0}")]
  Json(#[from] serde_json::Error),
}

impl Error {
  pub fn from_reason<S: Into<String>>(reason: S) -> Self {
    Self::AdHoc(reason.into())
  }
}

pub trait CallbackHandler {
  fn supported_callbacks(&self) -> &'static [&'static str];

  fn handle_callback(
    &self,
    name: &str,
    payload: String,
  ) -> Result<String, Error>;
}

/// A synchronous RPC channel that allows JavaScript to synchronously call out
/// to a child process and get a response over a line-based protocol,
/// including handling of JavaScript-side callbacks before the call completes.
///
/// #### Protocol
///
/// Requests follow a MessagePack-based "tuple"/array protocol with 3 items:
/// `(<type>, <name>, <payload>)`. All items are binary arrays of 8-bit
/// integers, including the `<type>` and `<name>`, to avoid unnecessary
/// encoding/decoding at the protocol level.
///
/// For specific message types and their corresponding protocol behavior, please
/// see `MessageType` below.
pub struct SyncRpcChannel<T> {
  child: Child,
  conn: RpcConnection<BufReader<ChildStdout>, BufWriter<ChildStdin>>,
  callback_handler: T,
  supported_callbacks: HashSet<&'static str>,
}

impl<T: CallbackHandler> SyncRpcChannel<T> {
  /// Constructs a new `SyncRpcChannel` by spawning a child process with the
  /// given `exe` executable, and a given set of `args`.
  pub fn new<I, S>(
    exe: impl AsRef<OsStr>,
    args: I,
    callback_handler: T,
  ) -> Result<Self, Error>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    let mut child = std::process::Command::new(exe)
      .stdin(std::process::Stdio::piped())
      .stdout(std::process::Stdio::piped())
      .stderr(std::process::Stdio::inherit())
      .args(args)
      .spawn()
      .map_err(Error::ProcessSpawn)?;
    let supported_callbacks = callback_handler.supported_callbacks();
    Ok(Self {
      conn: RpcConnection::new(
        BufReader::new(child.stdout.take().expect("Where did ChildStdout go?")),
        BufWriter::new(child.stdin.take().expect("Where did ChildStdin go?")),
      )
      .map_err(Error::RpcConnection)?,
      supported_callbacks: supported_callbacks.iter().copied().collect(),
      callback_handler,
      child,
    })
  }

  /// Send a request to the child process and wait for a response. The method
  /// will not return, synchronously, until a response is received or an error
  /// occurs.
  ///
  /// This method will take care of encoding and decoding the binary payload to
  /// and from a JS string automatically and suitable for smaller payloads.
  pub fn request_sync(
    &mut self,
    method: &str,
    payload: String,
  ) -> Result<String, Error> {
    self
      .request_bytes_sync(method, payload.as_bytes())
      .and_then(|arr| {
        String::from_utf8((&arr[..]).into()).map_err(|e| Error::Encoding {
          obj: "response",
          ty: "string",
          source: Box::new(Error::Utf8(e)),
        })
      })
  }

  /// Send a request to the child process and wait for a response. The method
  /// will not return, synchronously, until a response is received or an error
  /// occurs.
  ///
  /// Unlike `requestSync`, this method will not do any of its own encoding or
  /// decoding of payload data. Everything will be as sent/received through the
  /// underlying protocol.
  pub fn request_bytes_sync(
    &mut self,
    method: &str,
    payload: &[u8],
  ) -> Result<Vec<u8>, Error> {
    log::trace!("request_bytes_sync: {method}");
    let method_bytes = method.as_bytes();
    self
      .conn
      .write(MessageType::Request as u8, method_bytes, payload)
      .map_err(Error::RpcConnection)?;
    loop {
      let (ty, name, payload) =
        self.conn.read().map_err(Error::RpcConnection)?;
      match ty.try_into().map_err(Error::from_reason)? {
        MessageType::Response => {
          if name == method_bytes {
            return Ok(payload);
          } else {
            let name = String::from_utf8_lossy(&name);
            return Err(Error::from_reason(format!(
              "name mismatch for response: expected `{method}`, got `{name}`"
            )));
          }
        }
        MessageType::Error => {
          return Err(Error::RpcConnection(self.conn.create_error(
            &String::from_utf8_lossy(&name),
            payload,
            method,
          )));
        }
        MessageType::Call => {
          self.handle_call(&String::from_utf8_lossy(&name), payload)?;
        }
        _ => {
          return Err(Error::from_reason(format!(
            "Invalid message type from child: {ty:?}"
          )));
        }
      }
    }
  }

  // Closes the channel, terminating its underlying process.
  pub fn close(&mut self) -> Result<(), Error> {
    self.child.kill().map_err(Error::ProcessKill)?;
    Ok(())
  }

  // Helper method to handle callback calls
  fn handle_call(&mut self, name: &str, payload: Vec<u8>) -> Result<(), Error> {
    if !self.supported_callbacks.contains(name) {
      self.conn.write(MessageType::CallError as u8, name.as_bytes(), format!("unknown callback: `{name}`. Please make sure to register it on the JavaScript side before invoking it.").as_bytes())
                .map_err(Error::RpcConnection)?;
      return Err(Error::from_reason(format!(
        "no callback named `{name}` found"
      )));
    }
    let res = self
      .callback_handler
      .handle_callback(name, String::from_utf8(payload).map_err(Error::Utf8)?);
    match res {
      Ok(res) => {
        self
          .conn
          .write(
            MessageType::CallResponse as u8,
            name.as_bytes(),
            res.as_bytes(),
          )
          .map_err(Error::RpcConnection)?;
      }
      Err(e) => {
        self
          .conn
          .write(
            MessageType::CallError as u8,
            name.as_bytes(),
            format!("{e}").trim().as_bytes(),
          )
          .map_err(Error::RpcConnection)?;
        return Err(Error::from_reason(format!(
          "Error calling callback `{name}`: {}",
          e
        )));
      }
    }

    Ok(())
  }
}

/// Messages types exchanged between the channel and its child. All messages
/// have an associated `<name>` and `<payload>`, which will both be arrays of
/// 8-bit integers (`Uint8Array`s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
  // --- Sent by channel---
  /// A request to the child with the given raw byte `<payload>`, with
  /// `<name>` as the method name. The child may send back any number of
  /// `MessageType.Call` messages and must then close the request with either a
  /// `MessageType.Response`, or a `MessageType.Error`.  message.
  Request = 1,
  /// A response to a `MessageType.Call` message that the child previously sent.
  /// The `<payload>` is the return value from invoking the JavaScript callback
  /// associated with it. If the callback errors, `MessageType.CallError` will
  /// be sent to the child.
  CallResponse,
  /// Informs the child that an error occurred. The `<payload>` will be the
  /// binary representation of the stringified error, as UTF-8 bytes, not
  /// necessarily in JSON format. The method linked to this message will also
  /// throw an error after sending this message to its child and terminate the
  /// request call.
  CallError,

  // --- Sent by child ---
  /// A response to a request that the call was for. `<name>` MUST match the
  /// `MessageType.Request` message's `<name>` argument.
  Response,
  /// A response that denotes some error occurred while processing the request
  /// on the child side. The `<payload>` will simply be the binary
  /// representation of the stringified error, as UTF-8 bytes, not necessarily
  /// in JSON format. The method associated with this call will also throw an
  /// error after receiving this message from the child.
  Error,
  /// A request to invoke a pre-registered JavaScript callback (see
  /// `SyncRpcChannel#registerCallback`). `<name>` is the name of the callback,
  /// and `<payload>` is an encoded UTF-8 string that the callback will be
  /// called with. The child should then listen for `MessageType.CallResponse`
  /// and `MessageType.CallError` messages.
  Call,
  // NOTE: Do NOT put any variants below this one, always add them _before_ it.
  // See comment in TryFrom impl, and remove this when `variant_count` stabilizes.
  _UnusedPlaceholderVariant,
  // NOTHING SHOULD GO BELOW HERE
}

impl std::fmt::Display for MessageType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      MessageType::Request => write!(f, "MessageType::Request"),
      MessageType::CallResponse => write!(f, "MessageType::CallResponse"),
      MessageType::CallError => write!(f, "MessageType::CallError"),
      MessageType::Response => write!(f, "MessageType::Response"),
      MessageType::Error => write!(f, "MessageType::Error"),
      MessageType::Call => write!(f, "MessageType::Call"),
      MessageType::_UnusedPlaceholderVariant => {
        write!(f, "MessageType::_UnusedPlaceholderVariant")
      }
    }
  }
}

impl TryFrom<u8> for MessageType {
  type Error = String;

  fn try_from(
    value: u8,
  ) -> std::result::Result<Self, <MessageType as TryFrom<u8>>::Error> {
    // TODO: change to the following line when `variant_count` stabilizes
    // (https://github.com/rust-lang/rust/issues/73662) and remove `_UnusedPlaceholderVariant`
    //
    // if (1..=std::mem::variant_count::<MessageType>()) {
    if (1..(MessageType::_UnusedPlaceholderVariant as u8)).contains(&value) {
      // SAFETY: This is safe as long as the above holds true. It'll be fully
      // safe once `variant_count` stabilizes.
      Ok(unsafe { std::mem::transmute::<u8, MessageType>(value) })
    } else {
      Err(format!("Invalid message type: {value}"))
    }
  }
}
