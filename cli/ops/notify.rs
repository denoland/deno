use deno_core::plugin_api::Buf;
use deno_core::plugin_api::Interface;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ZeroCopyBuf;
use notify_rust::Notification;
use serde::{Deserialize, Serialize};


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
struct NotificationParams {
  title: String,
  message: String,
  icon: Option<Icon>,
  sound: Option<String>,
}

#[derive(Serialize)]
struct SendNotificationResult {}

fn op_notify_send(
    state: &mut OpState,
    args: Value,
    _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: SetEnv = serde_json::from_value(args)?;
  let mut notification = Notification::new();
  notification.summary(&params.title).body(&args.message);
  if let Some(icon_value) = &args.icon {
    notification.icon(match icon_value {
      Icon::App(app_name) => {=
        if let Err(error) = set_app_identifier(app_name) {
          response.err = Some(error);
        }
        app_name
      }
      Icon::Path(file_path) => file_path,
      Icon::Name(icon_name) => icon_name,
    });
  }
  if let Some(sound_name) = &params.sound {
    notification.sound_name(sound_name);
  }
  match notification.show() {
    Ok(_) => {
      Ok(json!({}))
    }
    Err(error) => {
      Ok(json!({error: error.to_string()})
    }
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
