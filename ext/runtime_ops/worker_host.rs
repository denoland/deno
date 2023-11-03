// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::op2;
use deno_core::serde::de;
use deno_core::serde::Deserialize;
use deno_core::serde::Deserializer;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_web::JsMessageData;
use log::debug;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

#[derive(
  Debug, Default, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct WorkerId(u32);
impl fmt::Display for WorkerId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "worker-{}", self.0)
  }
}
impl WorkerId {
  pub fn next(&self) -> Option<WorkerId> {
    self.0.checked_add(1).map(WorkerId)
  }
}

pub type FormatJsErrorFn = dyn Fn(&JsError) -> String + Sync + Send;

/// Events that are sent to host from child
/// worker.
pub enum WorkerControlEvent {
  Error(AnyError),
  TerminalError(AnyError),
  Close,
}

impl Serialize for WorkerControlEvent {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let type_id = match &self {
      WorkerControlEvent::TerminalError(_) => 1_i32,
      WorkerControlEvent::Error(_) => 2_i32,
      WorkerControlEvent::Close => 3_i32,
    };

    match self {
      WorkerControlEvent::TerminalError(error)
      | WorkerControlEvent::Error(error) => {
        let value = match error.downcast_ref::<JsError>() {
          Some(js_error) => {
            let frame = js_error.frames.iter().find(|f| match &f.file_name {
              Some(s) => !s.trim_start_matches('[').starts_with("ext:"),
              None => false,
            });
            json!({
              "message": js_error.exception_message,
              "fileName": frame.map(|f| f.file_name.as_ref()),
              "lineNumber": frame.map(|f| f.line_number.as_ref()),
              "columnNumber": frame.map(|f| f.column_number.as_ref()),
            })
          }
          None => json!({
            "message": error.to_string(),
          }),
        };

        Serialize::serialize(&(type_id, value), serializer)
      }
      _ => Serialize::serialize(&(type_id, ()), serializer),
    }
  }
}

#[async_trait::async_trait(?Send)]
pub trait WorkerHost {
  fn create_worker(
    state: &mut OpState,
    args: CreateWorkerArgs,
  ) -> Result<WorkerId, AnyError> {
    unimplemented!()
  }
  fn terminate_worker(state: &mut OpState, id: WorkerId) {
    unimplemented!()
  }
  fn post_message(
    state: &mut OpState,
    id: WorkerId,
    data: JsMessageData,
  ) -> Result<(), AnyError> {
    unimplemented!()
  }
  async fn recv_ctrl(
    state: Rc<RefCell<OpState>>,
    id: WorkerId,
  ) -> Result<WorkerControlEvent, AnyError> {
    unimplemented!()
  }
  async fn recv_message(
    state: Rc<RefCell<OpState>>,
    id: WorkerId,
  ) -> Result<Option<JsMessageData>, AnyError> {
    unimplemented!()
  }
}

pub const UNSTABLE_FEATURE_NAME: &str = "worker";

deno_core::extension!(
  deno_worker_host,
  parameters = [P: WorkerHost],
  ops = [
    op_create_worker<P>,
    op_host_terminate_worker<P>,
    op_host_post_message<P>,
    op_host_recv_ctrl<P>,
    op_host_recv_message<P>,
  ],
  state = |state| {
    state.put::<WorkerId>(WorkerId::default());
  },
);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebWorkerType {
  Classic,
  Module,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkerArgs {
  pub has_source_code: bool,
  pub name: Option<String>,
  pub permissions: Option<ChildPermissionsArg>,
  pub source_code: String,
  pub specifier: String,
  pub worker_type: WebWorkerType,
}

/// Create worker as the host
#[op2]
#[serde]
fn op_create_worker<H>(
  state: &mut OpState,
  #[serde] args: CreateWorkerArgs,
) -> Result<WorkerId, AnyError>
where
  H: WorkerHost + 'static,
{
  H::create_worker(state, args)
}

#[op2]
fn op_host_terminate_worker<H>(state: &mut OpState, #[serde] id: WorkerId)
where
  H: WorkerHost + 'static,
{
  H::terminate_worker(state, id)
}

/// Get control event from guest worker as host
#[op2(async)]
#[serde]
async fn op_host_recv_ctrl<H>(
  state: Rc<RefCell<OpState>>,
  #[serde] id: WorkerId,
) -> Result<WorkerControlEvent, AnyError>
where
  H: WorkerHost + 'static,
{
  H::recv_ctrl(state, id).await
}

#[op2(async)]
#[serde]
async fn op_host_recv_message<H>(
  state: Rc<RefCell<OpState>>,
  #[serde] id: WorkerId,
) -> Result<Option<JsMessageData>, AnyError>
where
  H: WorkerHost + 'static,
{
  H::recv_message(state, id).await
}

/// Post message to guest worker as host
#[op2]
fn op_host_post_message<H>(
  state: &mut OpState,
  #[serde] id: WorkerId,
  #[serde] data: JsMessageData,
) -> Result<(), AnyError>
where
  H: WorkerHost + 'static,
{
  H::post_message(state, id, data)
}

#[derive(Debug, Eq, PartialEq)]
pub enum ChildUnitPermissionArg {
  Inherit,
  Granted,
  NotGranted,
}

impl<'de> Deserialize<'de> for ChildUnitPermissionArg {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct ChildUnitPermissionArgVisitor;
    impl<'de> de::Visitor<'de> for ChildUnitPermissionArgVisitor {
      type Value = ChildUnitPermissionArg;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("\"inherit\" or boolean")
      }

      fn visit_unit<E>(self) -> Result<ChildUnitPermissionArg, E>
      where
        E: de::Error,
      {
        Ok(ChildUnitPermissionArg::NotGranted)
      }

      fn visit_str<E>(self, v: &str) -> Result<ChildUnitPermissionArg, E>
      where
        E: de::Error,
      {
        if v == "inherit" {
          Ok(ChildUnitPermissionArg::Inherit)
        } else {
          Err(de::Error::invalid_value(de::Unexpected::Str(v), &self))
        }
      }

      fn visit_bool<E>(self, v: bool) -> Result<ChildUnitPermissionArg, E>
      where
        E: de::Error,
      {
        match v {
          true => Ok(ChildUnitPermissionArg::Granted),
          false => Ok(ChildUnitPermissionArg::NotGranted),
        }
      }
    }
    deserializer.deserialize_any(ChildUnitPermissionArgVisitor)
  }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ChildUnaryPermissionArg {
  Inherit,
  Granted,
  NotGranted,
  GrantedList(Vec<String>),
}

impl<'de> Deserialize<'de> for ChildUnaryPermissionArg {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct ChildUnaryPermissionArgVisitor;
    impl<'de> de::Visitor<'de> for ChildUnaryPermissionArgVisitor {
      type Value = ChildUnaryPermissionArg;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("\"inherit\" or boolean or string[]")
      }

      fn visit_unit<E>(self) -> Result<ChildUnaryPermissionArg, E>
      where
        E: de::Error,
      {
        Ok(ChildUnaryPermissionArg::NotGranted)
      }

      fn visit_str<E>(self, v: &str) -> Result<ChildUnaryPermissionArg, E>
      where
        E: de::Error,
      {
        if v == "inherit" {
          Ok(ChildUnaryPermissionArg::Inherit)
        } else {
          Err(de::Error::invalid_value(de::Unexpected::Str(v), &self))
        }
      }

      fn visit_bool<E>(self, v: bool) -> Result<ChildUnaryPermissionArg, E>
      where
        E: de::Error,
      {
        match v {
          true => Ok(ChildUnaryPermissionArg::Granted),
          false => Ok(ChildUnaryPermissionArg::NotGranted),
        }
      }

      fn visit_seq<V>(
        self,
        mut v: V,
      ) -> Result<ChildUnaryPermissionArg, V::Error>
      where
        V: de::SeqAccess<'de>,
      {
        let mut granted_list = vec![];
        while let Some(value) = v.next_element::<String>()? {
          granted_list.push(value);
        }
        Ok(ChildUnaryPermissionArg::GrantedList(granted_list))
      }
    }
    deserializer.deserialize_any(ChildUnaryPermissionArgVisitor)
  }
}

/// Directly deserializable from JS worker and test permission options.
#[derive(Debug, Eq, PartialEq)]
pub struct ChildPermissionsArg {
  pub env: ChildUnaryPermissionArg,
  pub hrtime: ChildUnitPermissionArg,
  pub net: ChildUnaryPermissionArg,
  pub ffi: ChildUnaryPermissionArg,
  pub read: ChildUnaryPermissionArg,
  pub run: ChildUnaryPermissionArg,
  pub sys: ChildUnaryPermissionArg,
  pub write: ChildUnaryPermissionArg,
}

impl ChildPermissionsArg {
  pub fn inherit() -> Self {
    ChildPermissionsArg {
      env: ChildUnaryPermissionArg::Inherit,
      hrtime: ChildUnitPermissionArg::Inherit,
      net: ChildUnaryPermissionArg::Inherit,
      ffi: ChildUnaryPermissionArg::Inherit,
      read: ChildUnaryPermissionArg::Inherit,
      run: ChildUnaryPermissionArg::Inherit,
      sys: ChildUnaryPermissionArg::Inherit,
      write: ChildUnaryPermissionArg::Inherit,
    }
  }

  pub fn none() -> Self {
    ChildPermissionsArg {
      env: ChildUnaryPermissionArg::NotGranted,
      hrtime: ChildUnitPermissionArg::NotGranted,
      net: ChildUnaryPermissionArg::NotGranted,
      ffi: ChildUnaryPermissionArg::NotGranted,
      read: ChildUnaryPermissionArg::NotGranted,
      run: ChildUnaryPermissionArg::NotGranted,
      sys: ChildUnaryPermissionArg::NotGranted,
      write: ChildUnaryPermissionArg::NotGranted,
    }
  }
}

impl<'de> Deserialize<'de> for ChildPermissionsArg {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct ChildPermissionsArgVisitor;
    impl<'de> de::Visitor<'de> for ChildPermissionsArgVisitor {
      type Value = ChildPermissionsArg;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("\"inherit\" or \"none\" or object")
      }

      fn visit_unit<E>(self) -> Result<ChildPermissionsArg, E>
      where
        E: de::Error,
      {
        Ok(ChildPermissionsArg::inherit())
      }

      fn visit_str<E>(self, v: &str) -> Result<ChildPermissionsArg, E>
      where
        E: de::Error,
      {
        if v == "inherit" {
          Ok(ChildPermissionsArg::inherit())
        } else if v == "none" {
          Ok(ChildPermissionsArg::none())
        } else {
          Err(de::Error::invalid_value(de::Unexpected::Str(v), &self))
        }
      }

      fn visit_map<V>(self, mut v: V) -> Result<ChildPermissionsArg, V::Error>
      where
        V: de::MapAccess<'de>,
      {
        let mut child_permissions_arg = ChildPermissionsArg::none();
        while let Some((key, value)) =
          v.next_entry::<String, serde_json::Value>()?
        {
          if key == "env" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.env = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.env) {e}"))
            })?;
          } else if key == "hrtime" {
            let arg = serde_json::from_value::<ChildUnitPermissionArg>(value);
            child_permissions_arg.hrtime = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.hrtime) {e}"))
            })?;
          } else if key == "net" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.net = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.net) {e}"))
            })?;
          } else if key == "ffi" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.ffi = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.ffi) {e}"))
            })?;
          } else if key == "read" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.read = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.read) {e}"))
            })?;
          } else if key == "run" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.run = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.run) {e}"))
            })?;
          } else if key == "sys" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.sys = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.sys) {e}"))
            })?;
          } else if key == "write" {
            let arg = serde_json::from_value::<ChildUnaryPermissionArg>(value);
            child_permissions_arg.write = arg.map_err(|e| {
              de::Error::custom(format!("(deno.permissions.write) {e}"))
            })?;
          } else {
            return Err(de::Error::custom("unknown permission name"));
          }
        }
        Ok(child_permissions_arg)
      }
    }
    deserializer.deserialize_any(ChildPermissionsArgVisitor)
  }
}
