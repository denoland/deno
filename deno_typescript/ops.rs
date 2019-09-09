use crate::TSOpDispatcher;
use crate::TSState;
use deno::ErrBox;
use deno::ModuleSpecifier;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;

#[derive(Debug)]
pub struct WrittenFile {
  pub url: String,
  pub module_name: String,
  pub source_code: String,
}

pub struct OpReadFile;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadFileArgs {
  file_name: String,
  language_version: Option<i32>,
  should_create_new_source_file: bool,
}

impl TSOpDispatcher for OpReadFile {
  fn dispatch(&self, _s: &mut TSState, v: Value) -> Result<Value, ErrBox> {
    let v: ReadFileArgs = serde_json::from_value(v)?;
    let (module_name, source_code) = if v.file_name.starts_with("$asset$/") {
      let asset = v.file_name.replace("$asset$/", "");
      let source_code = crate::get_asset2(&asset)?.to_string();
      (asset, source_code)
    } else {
      assert!(!v.file_name.starts_with("$assets$"), "you meant $asset$");
      let module_specifier =
        ModuleSpecifier::resolve_url_or_path(&v.file_name)?;
      let path = module_specifier.as_url().to_file_path().unwrap();
      println!("cargo:rerun-if-changed={}", path.display());
      (
        module_specifier.as_str().to_string(),
        std::fs::read_to_string(&path)?,
      )
    };
    Ok(json!({
      "moduleName": module_name,
      "sourceCode": source_code,
    }))
  }

  const NAME: &'static str = "readFile";
}

pub struct OpWriteFile;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteFileArgs {
  file_name: String,
  data: String,
  module_name: String,
}

impl TSOpDispatcher for OpWriteFile {
  fn dispatch(&self, s: &mut TSState, v: Value) -> Result<Value, ErrBox> {
    let v: WriteFileArgs = serde_json::from_value(v)?;
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

  const NAME: &'static str = "writeFile";
}

pub struct OpResolveModuleNames;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveModuleNamesArgs {
  module_names: Vec<String>,
  containing_file: String,
}

impl TSOpDispatcher for OpResolveModuleNames {
  fn dispatch(&self, s: &mut TSState, v: Value) -> Result<Value, ErrBox> {
    let v: ResolveModuleNamesArgs = serde_json::from_value(v).unwrap();
    let mut resolved = Vec::<String>::new();
    let referrer = ModuleSpecifier::resolve_url_or_path(&v.containing_file)?;
    for specifier in v.module_names {
      let ms = match s.import_map.get(&specifier) {
        Some(module_name) => {
          dbg!(module_name);
          ModuleSpecifier::resolve_url_or_path(&module_name)?
        }
        None => ModuleSpecifier::resolve_import(&specifier, referrer.as_str())?,
      };
      resolved.push(ms.as_str().to_string());
    }
    Ok(json!(resolved))
  }

  const NAME: &'static str = "resolveModuleNames";
}

pub struct OpExit;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExitArgs {
  code: i32,
}

impl TSOpDispatcher for OpExit {
  fn dispatch(&self, s: &mut TSState, v: Value) -> Result<Value, ErrBox> {
    let v: ExitArgs = serde_json::from_value(v)?;
    s.exit_code = v.code;
    std::process::exit(v.code)
  }

  const NAME: &'static str = "exit";
}

pub struct OpEmitResult;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitResult {
  pub emit_skipped: bool,
  pub diagnostics: Vec<String>,
  pub emitted_files: Vec<String>,
}

impl TSOpDispatcher for OpEmitResult {
  fn dispatch(&self, s: &mut TSState, v: Value) -> Result<Value, ErrBox> {
    let v: EmitResult = serde_json::from_value(v)?;
    s.emit_result = Some(v);
    Ok(json!(true))
  }

  const NAME: &'static str = "emitResult";
}
