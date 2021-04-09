// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::fs_util::canonicalize_path;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;

/// The transpile options that are significant out of a user provided tsconfig
/// file, that we want to deserialize out of the final config for a transpile.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitConfigOptions {
  pub check_js: bool,
  pub emit_decorator_metadata: bool,
  pub imports_not_used_as_values: String,
  pub inline_source_map: bool,
  pub jsx: String,
  pub jsx_factory: String,
  pub jsx_fragment_factory: String,
}

/// A structure that represents a set of options that were ignored and the
/// path those options came from.
#[derive(Debug, Clone, PartialEq)]
pub struct IgnoredCompilerOptions {
  pub items: Vec<String>,
  pub maybe_path: Option<PathBuf>,
}

impl fmt::Display for IgnoredCompilerOptions {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut codes = self.items.clone();
    codes.sort();
    if let Some(path) = &self.maybe_path {
      write!(f, "Unsupported compiler options in \"{}\".\n  The following options were ignored:\n    {}", path.to_string_lossy(), codes.join(", "))
    } else {
      write!(f, "Unsupported compiler options provided.\n  The following options were ignored:\n    {}", codes.join(", "))
    }
  }
}

impl Serialize for IgnoredCompilerOptions {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    Serialize::serialize(&self.items, serializer)
  }
}

/// A static slice of all the compiler options that should be ignored that
/// either have no effect on the compilation or would cause the emit to not work
/// in Deno.
pub const IGNORED_COMPILER_OPTIONS: &[&str] = &[
  "allowSyntheticDefaultImports",
  "allowUmdGlobalAccess",
  "baseUrl",
  "declaration",
  "declarationMap",
  "downlevelIteration",
  "esModuleInterop",
  "emitDeclarationOnly",
  "importHelpers",
  "inlineSourceMap",
  "inlineSources",
  "module",
  "noEmitHelpers",
  "noErrorTruncation",
  "noLib",
  "noResolve",
  "outDir",
  "paths",
  "preserveConstEnums",
  "reactNamespace",
  "rootDir",
  "rootDirs",
  "skipLibCheck",
  "sourceMap",
  "sourceRoot",
  "target",
  "types",
  "useDefineForClassFields",
];

pub const IGNORED_RUNTIME_COMPILER_OPTIONS: &[&str] = &[
  "assumeChangesOnlyAffectDirectDependencies",
  "build",
  "charset",
  "composite",
  "diagnostics",
  "disableSizeLimit",
  "emitBOM",
  "extendedDiagnostics",
  "forceConsistentCasingInFileNames",
  "generateCpuProfile",
  "help",
  "incremental",
  "init",
  "isolatedModules",
  "listEmittedFiles",
  "listFiles",
  "mapRoot",
  "maxNodeModuleJsDepth",
  "moduleResolution",
  "newLine",
  "noEmit",
  "noEmitOnError",
  "out",
  "outDir",
  "outFile",
  "preserveSymlinks",
  "preserveWatchOutput",
  "pretty",
  "project",
  "resolveJsonModule",
  "showConfig",
  "skipDefaultLibCheck",
  "stripInternal",
  "traceResolution",
  "tsBuildInfoFile",
  "typeRoots",
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsConfigJson {
  compiler_options: Option<HashMap<String, Value>>,
  exclude: Option<Vec<String>>,
  extends: Option<String>,
  files: Option<Vec<String>>,
  include: Option<Vec<String>>,
  references: Option<Value>,
  type_acquisition: Option<Value>,
}

fn parse_compiler_options(
  compiler_options: &HashMap<String, Value>,
  maybe_path: Option<PathBuf>,
  is_runtime: bool,
) -> Result<(Value, Option<IgnoredCompilerOptions>), AnyError> {
  let mut filtered: HashMap<String, Value> = HashMap::new();
  let mut items: Vec<String> = Vec::new();

  for (key, value) in compiler_options.iter() {
    let key = key.as_str();
    if (!is_runtime && IGNORED_COMPILER_OPTIONS.contains(&key))
      || IGNORED_RUNTIME_COMPILER_OPTIONS.contains(&key)
    {
      items.push(key.to_string());
    } else {
      filtered.insert(key.to_string(), value.to_owned());
    }
  }
  let value = serde_json::to_value(filtered)?;
  let maybe_ignored_options = if !items.is_empty() {
    Some(IgnoredCompilerOptions { items, maybe_path })
  } else {
    None
  };

  Ok((value, maybe_ignored_options))
}

/// Take a string of JSONC, parse it and return a serde `Value` of the text.
/// The result also contains any options that were ignored.
pub fn parse_config(
  config_text: &str,
  path: &Path,
) -> Result<(Value, Option<IgnoredCompilerOptions>), AnyError> {
  assert!(!config_text.is_empty());
  let jsonc = jsonc_parser::parse_to_serde_value(config_text)?.unwrap();
  let config: TsConfigJson = serde_json::from_value(jsonc)?;

  if let Some(compiler_options) = config.compiler_options {
    parse_compiler_options(&compiler_options, Some(path.to_owned()), false)
  } else {
    Ok((json!({}), None))
  }
}

/// A structure for managing the configuration of TypeScript
#[derive(Debug, Clone)]
pub struct TsConfig(pub Value);

impl TsConfig {
  /// Create a new `TsConfig` with the base being the `value` supplied.
  pub fn new(value: Value) -> Self {
    TsConfig(value)
  }

  pub fn as_bytes(&self) -> Vec<u8> {
    let map = self.0.as_object().unwrap();
    let ordered: BTreeMap<_, _> = map.iter().collect();
    let value = json!(ordered);
    value.to_string().as_bytes().to_owned()
  }

  /// Return the value of the `checkJs` compiler option, defaulting to `false`
  /// if not present.
  pub fn get_check_js(&self) -> bool {
    if let Some(check_js) = self.0.get("checkJs") {
      check_js.as_bool().unwrap_or(false)
    } else {
      false
    }
  }

  pub fn get_declaration(&self) -> bool {
    if let Some(declaration) = self.0.get("declaration") {
      declaration.as_bool().unwrap_or(false)
    } else {
      false
    }
  }

  /// Merge a serde_json value into the configuration.
  pub fn merge(&mut self, value: &Value) {
    json_merge(&mut self.0, value);
  }

  /// Take an optional string representing a user provided TypeScript config file
  /// which was passed in via the `--config` compiler option and merge it with
  /// the configuration.  Returning the result which optionally contains any
  /// compiler options that were ignored.
  ///
  /// When there are options ignored out of the file, a warning will be written
  /// to stderr regarding the options that were ignored.
  pub fn merge_tsconfig(
    &mut self,
    maybe_path: Option<String>,
  ) -> Result<Option<IgnoredCompilerOptions>, AnyError> {
    if let Some(path) = maybe_path {
      let cwd = std::env::current_dir()?;
      let config_file = cwd.join(path);
      let config_path = canonicalize_path(&config_file).map_err(|_| {
        std::io::Error::new(
          std::io::ErrorKind::InvalidInput,
          format!(
            "Could not find the config file: {}",
            config_file.to_string_lossy()
          ),
        )
      })?;
      let config_text = std::fs::read_to_string(config_path.clone())?;
      let (value, maybe_ignored_options) =
        parse_config(&config_text, &config_path)?;
      json_merge(&mut self.0, &value);

      Ok(maybe_ignored_options)
    } else {
      Ok(None)
    }
  }

  /// Take a map of compiler options, filtering out any that are ignored, then
  /// merge it with the current configuration, returning any options that might
  /// have been ignored.
  pub fn merge_user_config(
    &mut self,
    user_options: &HashMap<String, Value>,
  ) -> Result<Option<IgnoredCompilerOptions>, AnyError> {
    let (value, maybe_ignored_options) =
      parse_compiler_options(user_options, None, true)?;
    json_merge(&mut self.0, &value);
    Ok(maybe_ignored_options)
  }
}

impl Serialize for TsConfig {
  /// Serializes inner hash map which is ordered by the key
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    Serialize::serialize(&self.0, serializer)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::serde_json::json;

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
    let config_path = PathBuf::from("/deno/tsconfig.json");
    let (options_value, ignored) =
      parse_config(config_text, &config_path).expect("error parsing");
    assert!(options_value.is_object());
    let options = options_value.as_object().unwrap();
    assert!(options.contains_key("strict"));
    assert_eq!(options.len(), 1);
    assert_eq!(
      ignored,
      Some(IgnoredCompilerOptions {
        items: vec!["build".to_string()],
        maybe_path: Some(config_path),
      }),
    );
  }

  #[test]
  fn test_tsconfig_merge_user_options() {
    let mut tsconfig = TsConfig::new(json!({
      "target": "esnext",
      "module": "esnext",
    }));
    let user_options = serde_json::from_value(json!({
      "target": "es6",
      "build": true,
      "strict": false,
    }))
    .expect("could not convert to hashmap");
    let maybe_ignored_options = tsconfig
      .merge_user_config(&user_options)
      .expect("could not merge options");
    assert_eq!(
      tsconfig.0,
      json!({
        "module": "esnext",
        "target": "es6",
        "strict": false,
      })
    );
    assert_eq!(
      maybe_ignored_options,
      Some(IgnoredCompilerOptions {
        items: vec!["build".to_string()],
        maybe_path: None
      })
    );
  }

  #[test]
  fn test_tsconfig_as_bytes() {
    let mut tsconfig1 = TsConfig::new(json!({
      "strict": true,
      "target": "esnext",
    }));
    tsconfig1.merge(&json!({
      "target": "es5",
      "module": "amd",
    }));
    let mut tsconfig2 = TsConfig::new(json!({
      "target": "esnext",
      "strict": true,
    }));
    tsconfig2.merge(&json!({
      "module": "amd",
      "target": "es5",
    }));
    assert_eq!(tsconfig1.as_bytes(), tsconfig2.as_bytes());
  }
}
