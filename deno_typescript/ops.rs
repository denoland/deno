use crate::TSState;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use deno_core::Op;
use ring::digest;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use std::result;

type OpResult = result::Result<Value, ErrBox>;
type Dispatcher = fn(state: &mut TSState, args: Value) -> OpResult;

#[derive(Clone, Debug)]
pub struct WrittenFile {
  pub module_name: String,
  pub url: String,
  pub source_code: String,
}

pub fn json_op(d: Dispatcher) -> impl Fn(&mut TSState, &[u8]) -> Op {
  move |state: &mut TSState, control: &[u8]| {
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadModule {
  module_url: String,
  language_version: Option<i32>,
  should_create_new_source_file: bool,
}

pub fn op_load_module(s: &mut TSState, v: Value) -> OpResult {
  let v: LoadModule = serde_json::from_value(v)?;
  let module_specifier = ModuleSpecifier::resolve_url_or_path(&v.module_url)?;
  let module_url = module_specifier.as_url();
  let (module_name, source_code) = match module_url.scheme() {
    "asset" => {
      let asset = v.module_url.replace("asset:///", "");
      let source_code = match crate::get_asset(&asset) {
        Some(code) => code.to_string(),
        None => {
          return Err(
            std::io::Error::new(
              std::io::ErrorKind::NotFound,
              "Asset not found",
            )
            .into(),
          );
        }
      };
      (asset, source_code)
    }
    "file" => {
      let path = module_url.to_file_path().unwrap();
      println!("cargo:rerun-if-changed={}", path.display());
      (
        module_specifier.as_str().to_string(),
        std::fs::read_to_string(&path)?,
      )
    }
    "crate" => {
      let crate_name = module_url.host_str().unwrap();
      // TODO(afinch7) turn failures here into real error messages.
      let path_prefix = s.extern_crate_modules.get(crate_name).unwrap();
      let path =
        std::path::Path::new(path_prefix).join(&module_url.path()[1..]);
      (
        module_specifier.as_str().to_string(),
        std::fs::read_to_string(&path)?,
      )
    }
    _ => unimplemented!(),
  };
  Ok(json!({
    "moduleName": module_name,
    "sourceCode": source_code,
    "hash": gen_hash(&[&source_code.as_bytes()])
  }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadFile {
  file_name: String,
}

pub fn op_read_file(_s: &mut TSState, v: Value) -> OpResult {
  let v: ReadFile = serde_json::from_value(v)?;
  let file_specifier = ModuleSpecifier::resolve_url_or_path(&v.file_name)?;
  let file_url = file_specifier.as_url();
  let data = match file_url.scheme() {
    "asset" => unimplemented!(),
    "file" => {
      let path = file_url.to_file_path().unwrap();
      if path.is_file() {
        println!("cargo:rerun-if-changed={}", path.display());
        Some(std::fs::read_to_string(&path)?)
      } else {
        None
      }
    }
    "crate" => unimplemented!(),
    _ => unimplemented!(),
  };
  Ok(json!({
    "data": data,
  }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteFile {
  file_name: String,
  data: String,
  maybe_module_name: Option<String>,
}

pub fn op_write_file(s: &mut TSState, v: Value) -> OpResult {
  let v: WriteFile = serde_json::from_value(v)?;
  let module_specifier = ModuleSpecifier::resolve_url_or_path(&v.file_name)?;
  let module_url = module_specifier.as_url();
  if module_url.scheme() == "file" {
    std::fs::write(&v.file_name, &v.data)?;
  }
  if let Some(module_name) = v.maybe_module_name {
    s.written_files.push(WrittenFile {
      module_name,
      url: module_specifier.as_str().to_string(),
      source_code: v.data,
    });
  }
  Ok(json!(true))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateHash {
  data: String,
}

pub fn op_create_hash(_s: &mut TSState, v: Value) -> OpResult {
  let v: CreateHash = serde_json::from_value(v)?;
  Ok(json!({ "hash": gen_hash(&[&v.data.as_bytes()]) }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveModuleNames {
  module_names: Vec<String>,
  containing_file: String,
}

pub fn op_resolve_module_names(_s: &mut TSState, v: Value) -> OpResult {
  let v: ResolveModuleNames = serde_json::from_value(v).unwrap();
  let mut resolved = Vec::<String>::new();
  let referrer = ModuleSpecifier::resolve_url_or_path(&v.containing_file)?;
  for specifier in v.module_names {
    if specifier.starts_with("asset:///") {
      resolved.push(specifier.clone());
    } else {
      let ms = ModuleSpecifier::resolve_import(&specifier, referrer.as_str())?;
      resolved.push(ms.as_str().to_string());
    }
  }
  Ok(json!(resolved))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Exit {
  code: i32,
}

pub fn op_exit2(s: &mut TSState, v: Value) -> OpResult {
  let v: Exit = serde_json::from_value(v)?;
  s.exit_code = v.code;
  std::process::exit(v.code)
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EmitResult {
  pub emit_skipped: bool,
  pub diagnostics: Vec<String>,
  pub emitted_files: Option<Vec<String>>,
  pub root_specifier: String,
}

pub fn op_set_emit_result(s: &mut TSState, v: Value) -> OpResult {
  let v: EmitResult = serde_json::from_value(v)?;
  s.emit_result = Some(v);
  Ok(json!(true))
}

/// Generate a SHA256 hash of the data passed and return it as a `String`
fn gen_hash(v: &[&[u8]]) -> String {
  let mut ctx = digest::Context::new(&digest::SHA256);
  for src in v {
    ctx.update(src);
  }
  let digest = ctx.finish();
  let out: Vec<String> = digest
    .as_ref()
    .iter()
    .map(|byte| format!("{:02x}", byte))
    .collect();
  out.join("")
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;

  fn setup() -> TSState {
    TSState {
      exit_code: 0,
      emit_result: None,
      written_files: Vec::new(),
      extern_crate_modules: HashMap::new(),
    }
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct LoadedModule {
    module_name: String,
    source_code: String,
    hash: String,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct HashResponse {
    hash: String,
  }

  #[test]
  fn test_gen_hash() {
    let actual = gen_hash(&[b"hello world"]);
    assert_eq!(
      actual,
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
  }

  #[test]
  fn test_op_load_module() {
    let mut state = setup();
    let value = serde_json::json!({
      "moduleUrl": "asset:///lib.esnext.d.ts",
      "languageVersion": 1,
      "shouldCreateNewSourceFile": false,
    });
    let response =
      op_load_module(&mut state, value).expect("failed to load module");
    let actual: LoadedModule = serde_json::from_value(response).unwrap();
    assert_eq!(actual.module_name, "lib.esnext.d.ts");
    assert_eq!(actual.source_code.len(), 999);
    assert_eq!(
      actual.hash,
      "feafcdd6f11ac9c4af7a20bb4420b3a98b37036c9290b19a413d4cb5244b1093"
    );
  }

  #[test]
  fn test_op_create_hash() {
    let mut state = setup();
    let value = serde_json::json!({
      "data": "hello world",
    });
    let response = op_create_hash(&mut state, value).unwrap();
    let actual: HashResponse = serde_json::from_value(response).unwrap();
    assert_eq!(
      actual.hash,
      "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
  }
}
