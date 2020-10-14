// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use notify_rust::Notification;
use serde::Deserialize;
use serde::Serialize;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_notify_send", op_notify_send);
}

#[derive(Deserialize)]
enum Icon {
  #[serde(rename = "app")]
  App(String),
  #[serde(rename = "path")]
  Path(String),
  #[serde(rename = "name")]
  Name(String),
}

#[derive(Deserialize)]
struct NotificationActions {
  action: String,
  title: String,
}

// Allow dead spec fields in NotificationOptions
// ...might be used later?
#[allow(dead_code)]
#[derive(Deserialize)]
struct NotificationOptions {
  body: Option<String>,
  icon: Option<Icon>,
  tag: Option<String>,
  badge: Option<String>,
  image: Option<String>,
  vibrate: Option<bool>,
  #[serde(rename = "requireInteraction")]
  require_interaction: Option<bool>,
  actions: Option<Vec<NotificationActions>>,
  silent: Option<bool>,
}

#[derive(Deserialize)]
struct NotificationParams {
  title: String,
  options: Option<NotificationOptions>,
}

#[derive(Serialize, Debug)]
struct NotificationEvent {
  kind: String,
  data: Option<Value>,
}

fn op_notify_send(
  _state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: NotificationParams = serde_json::from_value(args)?;
  let mut notification = Notification::new();
  notification.summary(&args.title);
  if let Some(options) = &args.options {
    if let Some(message_value) = &options.body {
      notification.body(&message_value);
    }
    if let Some(actions) = &options.actions {
      for action in actions.iter() {
        notification.action(&action.action, &action.title);
      }
    }
    if let Some(icon_value) = &options.icon {
      notification.icon(match icon_value {
        Icon::App(app_name) => {
          if let Err(error) = set_app_identifier(&app_name) {
            return Ok(json!(error.to_string()));
          }
          app_name
        }
        Icon::Path(file_path) => file_path,
        Icon::Name(icon_name) => icon_name,
      });
    }
  }
  match notification.show() {
    Ok(notif) => {
      #[cfg(not(target_os = "macos"))]
      notif.wait_for_action(|action| match action {
        "__closed" => println!("the notification was closed"),
        _ => (),
      });
      return Ok(json!({}));
    }
    Err(error) => return Ok(json!(error.to_string())),
  };
}

#[cfg(not(target_os = "macos"))]
fn set_app_identifier(_app_name: &String) -> Result<(), String> {
  Ok(())
}

#[cfg(target_os = "macos")]
fn set_app_identifier(app_name: &String) -> Result<(), String> {
  use notify_rust::{get_bundle_identifier_or_default, set_application};

  let app_id = get_bundle_identifier_or_default(app_name);
  if let Err(err) = set_application(&app_id).map_err(|f| format!("{}", f)) {
    Err(err)
  } else {
    Ok(())
  }
}
