// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::compiler::CompilerState;
use crate::compiler::EmitResult;
use crate::msg::as_ts_filename;
use crate::msg::from_ts_filename;
use crate::msg::EmittedFile;

use deno_core::CoreIsolateState;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use deno_core::Op;
use deno_core::ZeroCopyBuf;
use ring::digest;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use std::result;
use std::sync::Arc;
use std::sync::Mutex;

type OpResult = result::Result<Value, ErrBox>;
type Dispatcher = fn(state: &mut CompilerState, args: Value) -> OpResult;

pub fn compiler_op<D>(
  state: Arc<Mutex<CompilerState>>,
  dispatcher: D,
) -> impl Fn(&mut CoreIsolateState, &mut [ZeroCopyBuf]) -> Op
where
  D: Fn(&mut CompilerState, &[u8]) -> Op,
{
  move |_state: &mut CoreIsolateState,
        zero_copy_bufs: &mut [ZeroCopyBuf]|
        -> Op {
    assert_eq!(zero_copy_bufs.len(), 1, "Invalid number of arguments");
    let mut s = state.lock().unwrap();
    dispatcher(&mut s, &zero_copy_bufs[0])
  }
}

// Fn(&mut CoreIsolateState, &mut [ZeroCopyBuf]) -> Op + 'static

pub fn json_op(d: Dispatcher) -> impl Fn(&mut CompilerState, &[u8]) -> Op {
  move |state: &mut CompilerState, control: &[u8]| {
    let result = serde_json::from_slice(control)
      .map_err(ErrBox::from)
      .and_then(move |args| d(state, args));

    let response = match result {
      Ok(v) => json!({ "ok": v }),
      Err(err) => json!({ "err": err.to_string() }),
    };

    let x = serde_json::to_string(&response).unwrap();
    let vec = x.into_bytes();
    Op::Sync(vec.into_boxed_slice())
  }
}

/// Generate a SHA256 hash of the data passed and return it as a `String`
fn gen_hash(v: &[impl AsRef<[u8]>]) -> String {
  let mut ctx = digest::Context::new(&digest::SHA256);
  for src in v {
    ctx.update(src.as_ref());
  }
  let digest = ctx.finish();
  let out: Vec<String> = digest
    .as_ref()
    .iter()
    .map(|byte| format!("{:02x}", byte))
    .collect();
  out.join("")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateHash {
  data: String,
}

pub fn op_create_hash(s: &mut CompilerState, v: Value) -> OpResult {
  let v: CreateHash = serde_json::from_value(v)?;
  let mut hash_data = vec![v.data.as_bytes().to_owned()];
  hash_data.extend_from_slice(&s.hash_data);
  Ok(json!({ "hash": gen_hash(&hash_data) }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadModule {
  specifier: String,
}

pub fn op_load_module(s: &mut CompilerState, v: Value) -> OpResult {
  let v: LoadModule = serde_json::from_value(v)?;
  let module_specifier = from_ts_filename(&v.specifier, &s.maybe_shared_path)?;
  let data = if let Some(provider) = s.maybe_provider.as_ref() {
    if let Some(module) = provider.borrow().get_source(&module_specifier) {
      module
    } else {
      return Err(
        std::io::Error::new(
          std::io::ErrorKind::NotFound,
          format!("Resource not found. Specifier: {}", module_specifier),
        )
        .into(),
      );
    }
  } else if let Some(source) = s.sources.get(&module_specifier.to_string()) {
    source.clone()
  } else {
    let module_url = module_specifier.as_url();
    match module_url.scheme() {
      "asset" => {
        let file_name = v.specifier.replace("asset:///", "");
        let path = s.maybe_assets_path.clone().unwrap().join(file_name);
        let data = std::fs::read_to_string(path);
        data.unwrap()
      }
      _ => {
        return Err(
          std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Resource not found. Specifier: {}", module_specifier),
          )
          .into(),
        );
      }
    }
  };
  let mut hash_data = vec![data.as_bytes().to_owned()];
  hash_data.extend_from_slice(&s.hash_data);
  let hash = gen_hash(&hash_data);
  Ok(json!({ "data": data, "hash": hash }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadFile {
  file_name: String,
}

pub fn op_read_file(s: &mut CompilerState, v: Value) -> OpResult {
  let v: ReadFile = serde_json::from_value(v)?;
  let file_specifier = ModuleSpecifier::resolve_url_or_path(&v.file_name)?;
  let data = match file_specifier.as_str() {
    "cache:///.tsbuildinfo" => s.maybe_build_info.clone(),
    _ => None,
  };
  Ok(json!({
    "data": data,
  }))
}

/// A mapping function from a specifier to a ts.Extension that is used only for
/// specifiers where we don't have a media type provided (like internal sources)
fn get_ts_extension(specifier: &str) -> &str {
  if specifier.ends_with(".d.ts") {
    ".d.ts"
  } else if specifier.ends_with(".ts") {
    ".ts"
  } else if specifier.ends_with(".tsx") {
    ".tsx"
  } else if specifier.ends_with(".jsx") {
    ".jsx"
  } else if specifier.ends_with(".json") {
    ".json"
  } else if specifier.ends_with(".tsbuildinfo") {
    ".tsbuildinfo"
  } else {
    ".js"
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveSpecifiers {
  specifiers: Vec<String>,
  base: String,
}

pub fn op_resolve_specifiers(s: &mut CompilerState, v: Value) -> OpResult {
  let v: ResolveSpecifiers = serde_json::from_value(v).unwrap();
  let mut resolved: Vec<(String, String)> = Vec::new();
  let base = from_ts_filename(&v.base, &s.maybe_shared_path)?;
  for specifier in v.specifiers {
    if specifier.starts_with("asset:///") {
      resolved
        .push((specifier.clone(), get_ts_extension(&specifier).to_string()));
    } else if let Some(provider) = s.maybe_provider.as_ref() {
      let (ms, media_type) = provider.borrow().resolve(&specifier, &base)?;
      resolved.push((
        as_ts_filename(&ms, &s.maybe_shared_path),
        media_type.to_ts_extension(&ms).to_string(),
      ));
    } else {
      unreachable!();
    }
  }
  Ok(json!(resolved))
}

pub fn op_set_emit_result(s: &mut CompilerState, v: Value) -> OpResult {
  let v: EmitResult = serde_json::from_value(v)?;
  s.emit_result = Some(v);
  Ok(json!(true))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Version {
  version: String,
}

pub fn op_set_version(s: &mut CompilerState, v: Value) -> OpResult {
  let v: Version = serde_json::from_value(v)?;
  s.version = v.version;
  Ok(json!(true))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteFile {
  file_name: String,
  data: String,
  maybe_module_name: Option<String>,
}

pub fn op_write_file(s: &mut CompilerState, v: Value) -> OpResult {
  let v: WriteFile = serde_json::from_value(v)?;
  let module_specifier = from_ts_filename(&v.file_name, &s.maybe_shared_path)?;
  match module_specifier.as_str() {
    "cache:///.tsbuildinfo" => s.maybe_build_info = Some(v.data),
    _ => {
      s.written_files.push(EmittedFile {
        data: v.data,
        maybe_module_name: if let Some(module_name) = v.maybe_module_name {
          Some(
            from_ts_filename(&module_name, &s.maybe_shared_path)?
              .as_str()
              .to_string(),
          )
        } else {
          None
        },
        url: module_specifier.as_str().to_string(),
      });
    }
  }

  Ok(json!(true))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn setup() -> CompilerState {
    CompilerState {
      ..CompilerState::default()
    }
  }

  #[test]
  fn test_gen_hash() {
    let actual = gen_hash(&[b"hello world"]);
    assert_eq!(
      actual,
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct HashResponse {
    hash: String,
  }

  #[test]
  fn test_op_create_hash() {
    let mut state = setup();
    let value = json!({ "data": "hello world" });
    let response = op_create_hash(&mut state, value).unwrap();
    let actual: HashResponse = serde_json::from_value(response).unwrap();
    assert_eq!(
      actual.hash,
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
  }
}
