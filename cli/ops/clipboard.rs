use crate::permissions::Permissions;
use clipboard::ClipboardContext;
use clipboard::ClipboardProvider;
use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::{OpState, ZeroCopyBuf};
use serde::Deserialize;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_clipboard_create", op_clipboard_create);
  super::reg_json_sync(rt, "op_clipboard_read", op_clipboard_read);
  super::reg_json_sync(rt, "op_clipboard_write", op_clipboard_write);
}

fn op_clipboard_create(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let ctx: ClipboardContext = ClipboardProvider::new().unwrap();

  let rid = state.resource_table.add("clipboard", Box::new(ctx));

  Ok(json!(rid))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadArgs {
  rid: u32,
}

fn op_clipboard_read(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  state.borrow::<Permissions>().check_clipboard_read()?;

  let args: ReadArgs = serde_json::from_value(args)?;
  let ctx = state
    .resource_table
    .get_mut::<ClipboardContext>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let data = ctx.get_contents().unwrap();

  Ok(json!(data))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteArgs {
  rid: u32,
  content: String,
}

fn op_clipboard_write(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  state.borrow::<Permissions>().check_clipboard_write()?;

  let args: WriteArgs = serde_json::from_value(args)?;
  let ctx = state
    .resource_table
    .get_mut::<ClipboardContext>(args.rid)
    .ok_or_else(bad_resource_id)?;

  ctx.set_contents(args.content).unwrap();

  Ok(json!({}))
}
