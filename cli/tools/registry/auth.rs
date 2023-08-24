// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::bail;
use deno_core::error::AnyError;

use super::urls::REGISTRY_URL;

pub fn save_token(token: String) -> Result<(), AnyError> {
  let contents = vec![format!("deno-registry-token:{}", REGISTRY_URL), token];
  std::fs::write("./deno.token", contents.join("\n"))?;
  Ok(())
}

fn read_token() -> Result<Option<String>, AnyError> {
  let contents = std::fs::read_to_string("./deno.token")?;
  let Some((_url, token)) = contents.split_once('\n') else {
    return Ok(None)
  };
  Ok(Some(token.to_string()))
}

pub fn ensure_token() -> Result<String, AnyError> {
  let maybe_token = read_token()?;
  let Some(token) = maybe_token else {
    bail!("Not logged in. Use `deno reg login` and try again.");
  };
  Ok(token)
}
