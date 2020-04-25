use crate::TSState;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use deno_core::Op;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct WrittenFile {
  pub url: String,
  pub module_name: String,
  pub source_code: String,
}

type Dispatcher = fn(state: &mut TSState, args: Value) -> Result<Value, ErrBox>;

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

pub fn op_load_module(s: &mut TSState, v: Value) -> Result<Value, ErrBox> {
  let v: LoadModule = serde_json::from_value(v)?;
  let (module_name, source_code) = if v.module_url.starts_with("$asset$/") {
    let asset = v.module_url.replace("$asset$/", "");

    let source_code = match crate::get_asset(&asset) {
      Some(code) => code.to_string(),
      None => {
        return Err(
          std::io::Error::new(std::io::ErrorKind::NotFound, "Asset not found")
            .into(),
        );
      }
    };

    (asset, source_code)
  } else {
    assert!(!v.module_url.starts_with("$assets$"), "you meant $asset$");
    let module_specifier = ModuleSpecifier::resolve_url_or_path(&v.module_url)?;
    let module_url = module_specifier.as_url();
    match module_url.scheme() {
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
    }
  };
  Ok(json!({
    "moduleName": module_name,
    "sourceCode": source_code,
  }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteFile {
  file_name: String,
  data: String,
  module_name: String,
}

pub fn op_write_file(s: &mut TSState, v: Value) -> Result<Value, ErrBox> {
  let v: WriteFile = serde_json::from_value(v)?;
  let module_specifier = ModuleSpecifier::resolve_url_or_path(&v.file_name)?;
  if s.bundle {
    std::fs::write(&v.file_name, &v.data)?;
  }
  s.written_files.push(WrittenFile {
    url: module_specifier.as_str().to_string(),
    module_name: v.module_name,
    source_code: v.data,
  });
  Ok(json!(true))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveModuleNames {
  module_names: Vec<String>,
  containing_file: String,
}

pub fn op_resolve_module_names(
  _s: &mut TSState,
  v: Value,
) -> Result<Value, ErrBox> {
  let v: ResolveModuleNames = serde_json::from_value(v).unwrap();
  let mut resolved = Vec::<String>::new();
  let referrer = ModuleSpecifier::resolve_url_or_path(&v.containing_file)?;
  for specifier in v.module_names {
    if specifier.starts_with("$asset$/") {
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

pub fn op_exit2(s: &mut TSState, v: Value) -> Result<Value, ErrBox> {
  let v: Exit = serde_json::from_value(v)?;
  s.exit_code = v.code;
  std::process::exit(v.code)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitResult {
  pub emit_skipped: bool,
  pub diagnostics: Vec<String>,
  pub emitted_files: Vec<String>,
}

pub fn op_set_emit_result(s: &mut TSState, v: Value) -> Result<Value, ErrBox> {
  let v: EmitResult = serde_json::from_value(v)?;
  s.emit_result = Some(v);
  Ok(json!(true))
}
