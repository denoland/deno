// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::ErrBox;
use jsonc_parser::JsonValue;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq)]
pub struct IgnoredCompilerOptions(pub Vec<String>);

impl fmt::Display for IgnoredCompilerOptions {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut codes = self.0.clone();
    codes.sort();
    write!(f, "{}", codes.join(", "))?;

    Ok(())
  }
}

/// A static slice of all the compiler options that should be ignored that
/// either have no effect on the compilation or would cause the emit to not work
/// in Deno.
const IGNORED_COMPILER_OPTIONS: [&str; 61] = [
  "allowSyntheticDefaultImports",
  "allowUmdGlobalAccess",
  "assumeChangesOnlyAffectDirectDependencies",
  "baseUrl",
  "build",
  "composite",
  "declaration",
  "declarationDir",
  "declarationMap",
  "diagnostics",
  "downlevelIteration",
  "emitBOM",
  "emitDeclarationOnly",
  "esModuleInterop",
  "extendedDiagnostics",
  "forceConsistentCasingInFileNames",
  "generateCpuProfile",
  "help",
  "importHelpers",
  "incremental",
  "inlineSourceMap",
  "inlineSources",
  "init",
  "listEmittedFiles",
  "listFiles",
  "mapRoot",
  "maxNodeModuleJsDepth",
  "module",
  "moduleResolution",
  "newLine",
  "noEmit",
  "noEmitHelpers",
  "noEmitOnError",
  "noLib",
  "noResolve",
  "out",
  "outDir",
  "outFile",
  "paths",
  "preserveConstEnums",
  "preserveSymlinks",
  "preserveWatchOutput",
  "pretty",
  "reactNamespace",
  "resolveJsonModule",
  "rootDir",
  "rootDirs",
  "showConfig",
  "skipDefaultLibCheck",
  "skipLibCheck",
  "sourceMap",
  "sourceRoot",
  "stripInternal",
  "target",
  "traceResolution",
  "tsBuildInfoFile",
  "types",
  "typeRoots",
  "useDefineForClassFields",
  "version",
  "watch",
];

/// A function that works like JavaScript's `Object.assign()`.
pub fn json_merge(a: &mut Value, b: &Value) {
  match (a, b) {
    (&mut Value::Object(ref mut a), &Value::Object(ref b)) => {
      for (k, v) in b {
        json_merge(a.entry(k.clone()).or_insert(Value::Null), v);
      }
    }
    (a, b) => {
      *a = b.clone();
    }
  }
}

/// Convert a jsonc libraries `JsonValue` to a serde `Value`.
fn jsonc_to_serde(j: JsonValue) -> Value {
  match j {
    JsonValue::Array(arr) => {
      let vec = arr.into_iter().map(jsonc_to_serde).collect();
      Value::Array(vec)
    }
    JsonValue::Boolean(bool) => Value::Bool(bool),
    JsonValue::Null => Value::Null,
    JsonValue::Number(num) => {
      let number =
        serde_json::Number::from_str(&num).expect("could not parse number");
      Value::Number(number)
    }
    JsonValue::Object(obj) => {
      let mut map = serde_json::map::Map::new();
      for (key, json_value) in obj.into_iter() {
        map.insert(key, jsonc_to_serde(json_value));
      }
      Value::Object(map)
    }
    JsonValue::String(str) => Value::String(str),
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TSConfigJson {
  compiler_options: Option<HashMap<String, Value>>,
  exclude: Option<Vec<String>>,
  extends: Option<String>,
  files: Option<Vec<String>>,
  include: Option<Vec<String>>,
  references: Option<Value>,
  type_acquisition: Option<Value>,
}

pub fn parse_raw_config(config_text: &str) -> Result<Value, ErrBox> {
  assert!(!config_text.is_empty());
  let jsonc = jsonc_parser::parse_to_value(config_text)?.unwrap();
  Ok(jsonc_to_serde(jsonc))
}

/// Take a string of JSONC, parse it and return a serde `Value` of the text.
/// The result also contains any options that were ignored.
pub fn parse_config(
  config_text: &str,
) -> Result<(Value, Option<IgnoredCompilerOptions>), ErrBox> {
  assert!(!config_text.is_empty());
  let jsonc = jsonc_parser::parse_to_value(config_text)?.unwrap();
  let config: TSConfigJson = serde_json::from_value(jsonc_to_serde(jsonc))?;
  let mut compiler_options: HashMap<String, Value> = HashMap::new();
  let mut items: Vec<String> = Vec::new();

  if let Some(in_compiler_options) = config.compiler_options {
    for (key, value) in in_compiler_options.iter() {
      if IGNORED_COMPILER_OPTIONS.contains(&key.as_str()) {
        items.push(key.to_owned());
      } else {
        compiler_options.insert(key.to_owned(), value.to_owned());
      }
    }
  }
  let options_value = serde_json::to_value(compiler_options)?;
  let ignored_options = if !items.is_empty() {
    Some(IgnoredCompilerOptions(items))
  } else {
    None
  };

  Ok((options_value, ignored_options))
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn test_json_merge() {
    let mut value_a = json!({
      "a": true,
      "b": "c"
    });
    let value_b = json!({
      "b": "d",
      "e": false,
    });
    json_merge(&mut value_a, &value_b);
    assert_eq!(
      value_a,
      json!({
        "a": true,
        "b": "d",
        "e": false,
      })
    );
  }

  #[test]
  fn test_parse_config() {
    let config_text = r#"{
      "compilerOptions": {
        "build": true,
        // comments are allowed
        "strict": true
      }
    }"#;
    let (options_value, ignored) =
      parse_config(config_text).expect("error parsing");
    assert!(options_value.is_object());
    let options = options_value.as_object().unwrap();
    assert!(options.contains_key("strict"));
    assert_eq!(options.len(), 1);
    assert_eq!(
      ignored,
      Some(IgnoredCompilerOptions(vec!["build".to_string()])),
    );
  }

  #[test]
  fn test_parse_raw_config() {
    let invalid_config_text = r#"{
      "compilerOptions": {
        // comments are allowed
    }"#;
    let errbox = parse_raw_config(invalid_config_text).unwrap_err();
    assert!(errbox
      .to_string()
      .starts_with("Unterminated object on line 1"));
  }
}
