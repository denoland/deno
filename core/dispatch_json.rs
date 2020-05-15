// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::isolate::ZeroCopyBuf;
use crate::ops::Buf;
use crate::ops::Op;
use crate::plugin_api::DispatchOpFn;
use crate::plugin_api::Interface;
use futures::future::FutureExt;
pub use serde_derive::{Deserialize, Serialize};
use serde_json::json;
pub use serde_json::Value;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

/// Error kind representation for all errors.
/// For encoding kinds are split into three distinct regions:
/// internal(<0), external(>0), and UnKind/other(==0).
///  ========================================================================
///  i64 Min <--- Internal Kinds ---< -1|0|1 >--- External Kinds ---> i64 Max
///                                      ^
///                                UnKind/Other
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum JsonErrorKind {
  /// Normal error kind.
  /// This value should always be greater than 0!
  /// These values are meaningless outside of their respective implementation
  /// of dispatch json(see full explanation in JsonError::kind).
  Kind(u32),
  /// No kind/not implemented. Corresponds to kind = 0.
  UnKind,
}

impl From<u32> for JsonErrorKind {
  fn from(kind: u32) -> Self {
    if kind == 0 {
      return Self::UnKind;
    }
    Self::Kind(kind)
  }
}

/// Object that can be encoded to json and sent back to represent rust errors to TS/JS.
pub trait JsonError: Debug + Sync + Send {
  /// Returns kind represented as a JsonErrorKind. The numeric values assoiacted
  /// to each kind are arbitrary(see explaination below).
  ///
  /// Their isn't a simple way to driectly share kind types between TS and rust.
  /// It is up to the implementation to control how error kinds are mapped to
  /// u32 values on the rust side and then converted back on the TS/JS side.
  /// On the rust side this is pretty much free form, but I recommend using
  /// a enum with discriminant values(see `InternalErrorKinds`). For the ts side
  /// this is controlled by the "errorFactory":
  /// (kind as u32, message) -> errorFactory -> Error
  fn kind(&self) -> JsonErrorKind;
  /// Error message as string value.
  fn msg(&self) -> String;
}

pub type JsonResult<E> = Result<Value, E>;

pub type AsyncJsonOp<E> = Pin<Box<dyn Future<Output = JsonResult<E>>>>;

pub enum JsonOp<E: JsonError> {
  Sync(Value),
  Async(AsyncJsonOp<E>),
  /// AsyncUnref is the variation of Async, which doesn't block the program
  /// exiting.
  AsyncUnref(AsyncJsonOp<E>),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AsyncArgs {
  promise_id: Option<u64>,
}

macro_rules! json_op_closure {
  ( $x:ty, $d:ident ) => {
    move |c: $x, control: &[u8], zero_copy: Option<ZeroCopyBuf>| {
      let async_args: AsyncArgs = match serde_json::from_slice(control) {
        Ok(args) => args,
        Err(e) => {
          let buf =
            serialize_result(None, Err(InternalError::to_final_json_error(&e)));
          return Op::Sync(buf);
        }
      };
      let promise_id = async_args.promise_id;
      let is_sync = promise_id.is_none();

      let result = serde_json::from_slice(control)
        .map_err(|e| e.to_final_json_error())
        .and_then(|args| $d(c, args, zero_copy).map_err(FinalJsonError::from));

      // Convert to Op
      match result {
        Ok(JsonOp::Sync(sync_value)) => {
          assert!(promise_id.is_none());
          Op::Sync(serialize_result(promise_id, Ok(sync_value)))
        }
        Ok(JsonOp::Async(fut)) => {
          assert!(promise_id.is_some());
          let fut2 = fut.then(move |result| {
            futures::future::ready(serialize_result(
              promise_id,
              result.map_err(FinalJsonError::from),
            ))
          });
          Op::Async(fut2.boxed_local())
        }
        Ok(JsonOp::AsyncUnref(fut)) => {
          assert!(promise_id.is_some());
          let fut2 = fut.then(move |result| {
            futures::future::ready(serialize_result(
              promise_id,
              result.map_err(FinalJsonError::from),
            ))
          });
          Op::AsyncUnref(fut2.boxed_local())
        }
        Err(sync_err) => {
          let buf = serialize_result(promise_id, Err(sync_err));
          if is_sync {
            Op::Sync(buf)
          } else {
            Op::Async(futures::future::ready(buf).boxed_local())
          }
        }
      }
    }
  };
}

/// This is the primary utility for dispatch_json it converts a json op function
/// into somthing that can be passed to op registration.
pub fn json_op<C, D, E>(
  d: D,
) -> impl Fn(&mut C, &[u8], Option<ZeroCopyBuf>) -> Op
where
  E: JsonError + 'static,
  D: Fn(&mut C, Value, Option<ZeroCopyBuf>) -> Result<JsonOp<E>, E>,
{
  json_op_closure!(&mut C, d)
}

/// Slightly different version of json_op for plugin ops.
pub fn plugin_json_op<D, E>(d: D) -> Box<dyn DispatchOpFn>
where
  E: JsonError + 'static,
  D: Fn(&mut dyn Interface, Value, Option<ZeroCopyBuf>) -> Result<JsonOp<E>, E>
    + 'static,
{
  Box::new(json_op_closure!(&mut dyn Interface, d))
}

pub fn blocking_json<F, E>(is_sync: bool, f: F) -> Result<JsonOp<E>, E>
where
  E: 'static + JsonError,
  F: 'static + Send + FnOnce() -> JsonResult<E>,
{
  if is_sync {
    Ok(JsonOp::Sync(f()?))
  } else {
    let fut = async move { tokio::task::spawn_blocking(f).await.unwrap() };
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}

// Private/Internal stuff

/// Internal representation of an error from inside or outside of json dispatch.
/// All errors get converted to this format before being serialized.
#[derive(Debug, Clone, Serialize)]
struct FinalJsonError {
  pub kind: i64,
  pub message: String,
}

impl<E: JsonError + Send + Sync> From<E> for FinalJsonError {
  fn from(e: E) -> FinalJsonError {
    let kind = match e.kind() {
      JsonErrorKind::Kind(k) => {
        assert!(k != 0, "kind = 0 is reserved for UnKind");
        k as i64
      }
      JsonErrorKind::UnKind => 0,
    };
    debug!("Converting JsonError({:?}) kind: {}", e, kind);
    FinalJsonError {
      kind,
      message: e.msg(),
    }
  }
}

fn serialize_result(
  promise_id: Option<u64>,
  result: Result<Value, FinalJsonError>,
) -> Buf {
  let value = match result {
    Ok(v) => json!({ "ok": v, "promiseId": promise_id }),
    Err(err) => json!({ "err": err, "promiseId": promise_id }),
  };
  serde_json::to_vec(&value).unwrap().into_boxed_slice()
}

/// Internal error kinds for json dispatch system
/// These values are serialized as negative(x * -1)!
// Warning! The values in this enum are duplicated in dispatch_json.ts
// Update carefully
#[derive(Clone, Copy, PartialEq, Debug)]
enum InternalErrorKinds {
  Io = 1,
  Syntax = 2,
  Data = 3,
  Eof = 4,
}

/// Internal error expression and conversion implementation.
trait InternalError {
  /// Kind is specific to implementation.
  fn kind(&self) -> JsonErrorKind;
  /// Error message as string value.
  fn msg(&self) -> String;
  /// Make InternalJsonError from this object
  fn to_final_json_error(&self) -> FinalJsonError {
    let kind = match self.kind() {
      JsonErrorKind::Kind(k) => {
        assert!(k != 0, "kind = 0 is reserved for UnKind");
        -(k as i64)
      }
      JsonErrorKind::UnKind => 0,
    };
    FinalJsonError {
      kind,
      message: self.msg(),
    }
  }
}

impl InternalError for serde_json::error::Error {
  fn kind(&self) -> JsonErrorKind {
    use serde_json::error::Category::*;
    JsonErrorKind::Kind(match self.classify() {
      Io => InternalErrorKinds::Io,
      Syntax => InternalErrorKinds::Syntax,
      Data => InternalErrorKinds::Data,
      Eof => InternalErrorKinds::Eof,
    } as u32)
  }

  fn msg(&self) -> String {
    self.to_string()
  }
}
