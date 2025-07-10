// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_error::JsErrorBox;

use crate::tools::deploy::get_token_entry;

deno_core::extension!(
  deno_deploy,
  ops = [
    op_deploy_token_get,
    op_deploy_token_set,
    op_deploy_token_delete,
  ],
);

#[op2]
#[string]
pub fn op_deploy_token_get() -> Result<Option<String>, JsErrorBox> {
  match get_token_entry()
    .map_err(|e| JsErrorBox::type_error(e.to_string()))?
    .get_password()
  {
    Ok(password) => Ok(Some(password)),
    Err(keyring::Error::NoEntry) => Ok(None),
    Err(e) => Err(JsErrorBox::type_error(e.to_string())),
  }
}

#[op2(fast)]
#[string]
pub fn op_deploy_token_set(#[string] s: &str) -> Result<(), JsErrorBox> {
  get_token_entry()
    .map_err(|e| JsErrorBox::type_error(e.to_string()))?
    .set_password(s)
    .map_err(|e| JsErrorBox::type_error(e.to_string()))
}

#[op2(fast)]
#[string]
pub fn op_deploy_token_delete() -> Result<(), JsErrorBox> {
  get_token_entry()
    .map_err(|e| JsErrorBox::type_error(e.to_string()))?
    .delete_credential()
    .map_err(|e| JsErrorBox::type_error(e.to_string()))
}
