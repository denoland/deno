// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::ConfigFlag;
use crate::args::Flags;
use crate::args::TaskFlags;
use crate::fs_util;
use crate::fs_util::canonicalize_path;
use crate::fs_util::specifier_parent;
use crate::fs_util::specifier_to_file_path;

use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;

pub type MaybeImportsResult =
  Result<Option<Vec<(ModuleSpecifier, Vec<String>)>>, AnyError>;

/// The transpile options that are significant out of a user provided tsconfig
/// file, that we want to deserialize out of the final config for a transpile.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitConfigOptions {
  pub check_js: bool,
  pub emit_decorator_metadata: bool,
  pub imports_not_used_as_values: String,
  pub inline_source_map: bool,
  pub inline_sources: bool,
  pub source_map: bool,
  pub jsx: String,
  pub jsx_factory: String,
  pub jsx_fragment_factory: String,
  pub jsx_import_source: Option<String>,
}

/// There are certain compiler options that can impact what modules are part of
/// a module graph, which need to be deserialized into a structure for analysis.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilerOptions {
  pub jsx: Option<String>,
  pub jsx_import_source: Option<String>,
  pub types: Option<Vec<String>>,
}

/// A structure that represents a set of options that were ignored and the
/// path those options came from.
#[derive(Debug, Clone, PartialEq)]
pub struct IgnoredCompilerOptions {
  pub items: Vec<String>,
  pub maybe_specifier: Option<ModuleSpecifier>,
}

impl fmt::Display for IgnoredCompilerOptions {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut codes = self.items.clone();
    codes.sort();
    if let Some(specifier) = &self.maybe_specifier {
      write!(f, "Unsupported compiler options in \"{}\".\n  The following options were ignored:\n    {}", specifier, codes.join(", "))
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
  "assumeChangesOnlyAffectDirectDependencies",
  "baseUrl",
  "build",
  "charset",
  "composite",
  "declaration",
  "declarationMap",
  "diagnostics",
  "disableSizeLimit",
  "downlevelIteration",
  "emitBOM",
  "emitDeclarationOnly",
  "esModuleInterop",
  "experimentalDecorators",
  "extendedDiagnostics",
  "forceConsistentCasingInFileNames",
  "generateCpuProfile",
  "help",
  "importHelpers",
  "incremental",
  "init",
  "inlineSourceMap",
  "inlineSources",
  "isolatedModules",
  "listEmittedFiles",
  "listFiles",
  "mapRoot",
  "maxNodeModuleJsDepth",
  "module",
  "moduleDetection",
  "moduleResolution",
  "newLine",
  "noEmit",
  "noEmitHelpers",
  "noEmitOnError",
  "noErrorTruncation",
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
  "project",
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

fn parse_compiler_options(
  compiler_options: &HashMap<String, Value>,
  maybe_specifier: Option<ModuleSpecifier>,
) -> Result<(Value, Option<IgnoredCompilerOptions>), AnyError> {
  let mut filtered: HashMap<String, Value> = HashMap::new();
  let mut items: Vec<String> = Vec::new();

  for (key, value) in compiler_options.iter() {
    let key = key.as_str();
    if IGNORED_COMPILER_OPTIONS.contains(&key) {
      items.push(key.to_string());
    } else {
      filtered.insert(key.to_string(), value.to_owned());
    }
  }
  let value = serde_json::to_value(filtered)?;
  let maybe_ignored_options = if !items.is_empty() {
    Some(IgnoredCompilerOptions {
      items,
      maybe_specifier,
    })
  } else {
    None
  };

  Ok((value, maybe_ignored_options))
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

  /// Take an optional user provided config file
  /// which was passed in via the `--config` flag and merge `compilerOptions` with
  /// the configuration.  Returning the result which optionally contains any
  /// compiler options that were ignored.
  pub fn merge_tsconfig_from_config_file(
    &mut self,
    maybe_config_file: Option<&ConfigFile>,
  ) -> Result<Option<IgnoredCompilerOptions>, AnyError> {
    if let Some(config_file) = maybe_config_file {
      let (value, maybe_ignored_options) = config_file.to_compiler_options()?;
      self.merge(&value);
      Ok(maybe_ignored_options)
    } else {
      Ok(None)
    }
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

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LintRulesConfig {
  pub tags: Option<Vec<String>>,
  pub include: Option<Vec<String>>,
  pub exclude: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SerializedFilesConfig {
  pub include: Vec<String>,
  pub exclude: Vec<String>,
}

impl SerializedFilesConfig {
  pub fn into_resolved(
    self,
    config_file_specifier: &ModuleSpecifier,
  ) -> Result<FilesConfig, AnyError> {
    let config_dir = specifier_parent(config_file_specifier);
    Ok(FilesConfig {
      include: self
        .include
        .into_iter()
        .map(|p| config_dir.join(&p))
        .collect::<Result<Vec<ModuleSpecifier>, _>>()?,
      exclude: self
        .exclude
        .into_iter()
        .map(|p| config_dir.join(&p))
        .collect::<Result<Vec<ModuleSpecifier>, _>>()?,
    })
  }
}

#[derive(Clone, Debug, Default)]
pub struct FilesConfig {
  pub include: Vec<ModuleSpecifier>,
  pub exclude: Vec<ModuleSpecifier>,
}

impl FilesConfig {
  /// Gets if the provided specifier is allowed based on the includes
  /// and excludes in the configuration file.
  pub fn matches_specifier(&self, specifier: &ModuleSpecifier) -> bool {
    // Skip files which is in the exclude list.
    let specifier_text = specifier.as_str();
    if self
      .exclude
      .iter()
      .any(|i| specifier_text.starts_with(i.as_str()))
    {
      return false;
    }

    // Ignore files not in the include list if it's not empty.
    self.include.is_empty()
      || self
        .include
        .iter()
        .any(|i| specifier_text.starts_with(i.as_str()))
  }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SerializedLintConfig {
  pub rules: LintRulesConfig,
  pub files: SerializedFilesConfig,
}

impl SerializedLintConfig {
  pub fn into_resolved(
    self,
    config_file_specifier: &ModuleSpecifier,
  ) -> Result<LintConfig, AnyError> {
    Ok(LintConfig {
      rules: self.rules,
      files: self.files.into_resolved(config_file_specifier)?,
    })
  }
}

#[derive(Clone, Debug, Default)]
pub struct LintConfig {
  pub rules: LintRulesConfig,
  pub files: FilesConfig,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum ProseWrap {
  Always,
  Never,
  Preserve,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct FmtOptionsConfig {
  pub use_tabs: Option<bool>,
  pub line_width: Option<u32>,
  pub indent_width: Option<u8>,
  pub single_quote: Option<bool>,
  pub prose_wrap: Option<ProseWrap>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SerializedFmtConfig {
  pub options: FmtOptionsConfig,
  pub files: SerializedFilesConfig,
}

impl SerializedFmtConfig {
  pub fn into_resolved(
    self,
    config_file_specifier: &ModuleSpecifier,
  ) -> Result<FmtConfig, AnyError> {
    Ok(FmtConfig {
      options: self.options,
      files: self.files.into_resolved(config_file_specifier)?,
    })
  }
}

#[derive(Clone, Debug, Default)]
pub struct FmtConfig {
  pub options: FmtOptionsConfig,
  pub files: FilesConfig,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SerializedTestConfig {
  pub files: SerializedFilesConfig,
}

impl SerializedTestConfig {
  pub fn into_resolved(
    self,
    config_file_specifier: &ModuleSpecifier,
  ) -> Result<TestConfig, AnyError> {
    Ok(TestConfig {
      files: self.files.into_resolved(config_file_specifier)?,
    })
  }
}

#[derive(Clone, Debug, Default)]
pub struct TestConfig {
  pub files: FilesConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFileJson {
  pub compiler_options: Option<Value>,
  pub import_map: Option<String>,
  pub lint: Option<Value>,
  pub fmt: Option<Value>,
  pub tasks: Option<Value>,
  pub test: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct ConfigFile {
  pub specifier: ModuleSpecifier,
  pub json: ConfigFileJson,
}

impl ConfigFile {
  pub fn discover(flags: &Flags) -> Result<Option<ConfigFile>, AnyError> {
    match &flags.config_flag {
      ConfigFlag::Disabled => Ok(None),
      ConfigFlag::Path(config_path) => Ok(Some(ConfigFile::read(config_path)?)),
      ConfigFlag::Discover => {
        if let Some(config_path_args) = flags.config_path_args() {
          let mut checked = HashSet::new();
          for f in config_path_args {
            if let Some(cf) = Self::discover_from(&f, &mut checked)? {
              return Ok(Some(cf));
            }
          }
          // attempt to resolve the config file from the task subcommand's
          // `--cwd` when specified
          if let crate::args::DenoSubcommand::Task(TaskFlags {
            cwd: Some(path),
            ..
          }) = &flags.subcommand
          {
            let task_cwd = fs_util::canonicalize_path(&PathBuf::from(path))?;
            if let Some(path) = Self::discover_from(&task_cwd, &mut checked)? {
              return Ok(Some(path));
            }
          };
          // From CWD walk up to root looking for deno.json or deno.jsonc
          let cwd = std::env::current_dir()?;
          Self::discover_from(&cwd, &mut checked)
        } else {
          Ok(None)
        }
      }
    }
  }

  pub fn discover_from(
    start: &Path,
    checked: &mut HashSet<PathBuf>,
  ) -> Result<Option<ConfigFile>, AnyError> {
    /// Filenames that Deno will recognize when discovering config.
    const CONFIG_FILE_NAMES: [&str; 2] = ["deno.json", "deno.jsonc"];

    for ancestor in start.ancestors() {
      if checked.insert(ancestor.to_path_buf()) {
        for config_filename in CONFIG_FILE_NAMES {
          let f = ancestor.join(config_filename);
          match ConfigFile::read(f) {
            Ok(cf) => {
              return Ok(Some(cf));
            }
            Err(e) => {
              if let Some(ioerr) = e.downcast_ref::<std::io::Error>() {
                use std::io::ErrorKind::*;
                match ioerr.kind() {
                  InvalidInput | PermissionDenied | NotFound => {
                    // ok keep going
                  }
                  _ => {
                    return Err(e); // Unknown error. Stop.
                  }
                }
              } else {
                return Err(e); // Parse error or something else. Stop.
              }
            }
          }
        }
      }
    }
    // No config file found.
    Ok(None)
  }

  pub fn read(path_ref: impl AsRef<Path>) -> Result<Self, AnyError> {
    let path = Path::new(path_ref.as_ref());
    let config_file = if path.is_absolute() {
      path.to_path_buf()
    } else {
      std::env::current_dir()?.join(path_ref)
    };

    let config_path = canonicalize_path(&config_file).map_err(|_| {
      std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!(
          "Could not find the config file: {}",
          config_file.to_string_lossy()
        ),
      )
    })?;
    let config_specifier = ModuleSpecifier::from_file_path(&config_path)
      .map_err(|_| {
        anyhow!(
          "Could not convert path to specifier. Path: {}",
          config_path.display()
        )
      })?;
    Self::from_specifier(&config_specifier)
  }

  pub fn from_specifier(specifier: &ModuleSpecifier) -> Result<Self, AnyError> {
    let config_path = specifier_to_file_path(specifier)?;
    let config_text = match std::fs::read_to_string(&config_path) {
      Ok(text) => text,
      Err(err) => bail!(
        "Error reading config file {}: {}",
        specifier,
        err.to_string()
      ),
    };
    Self::new(&config_text, specifier)
  }

  pub fn new(
    text: &str,
    specifier: &ModuleSpecifier,
  ) -> Result<Self, AnyError> {
    let jsonc =
      match jsonc_parser::parse_to_serde_value(text, &Default::default()) {
        Ok(None) => json!({}),
        Ok(Some(value)) if value.is_object() => value,
        Ok(Some(_)) => {
          return Err(anyhow!(
            "config file JSON {:?} should be an object",
            specifier,
          ))
        }
        Err(e) => {
          return Err(anyhow!(
            "Unable to parse config file JSON {:?} because of {}",
            specifier,
            e.to_string()
          ))
        }
      };
    let json: ConfigFileJson = serde_json::from_value(jsonc)?;

    Ok(Self {
      specifier: specifier.to_owned(),
      json,
    })
  }

  /// Returns true if the configuration indicates that JavaScript should be
  /// type checked, otherwise false.
  pub fn get_check_js(&self) -> bool {
    self
      .json
      .compiler_options
      .as_ref()
      .and_then(|co| co.get("checkJs").and_then(|v| v.as_bool()))
      .unwrap_or(false)
  }

  /// Parse `compilerOptions` and return a serde `Value`.
  /// The result also contains any options that were ignored.
  pub fn to_compiler_options(
    &self,
  ) -> Result<(Value, Option<IgnoredCompilerOptions>), AnyError> {
    if let Some(compiler_options) = self.json.compiler_options.clone() {
      let options: HashMap<String, Value> =
        serde_json::from_value(compiler_options)
          .context("compilerOptions should be an object")?;
      parse_compiler_options(&options, Some(self.specifier.to_owned()))
    } else {
      Ok((json!({}), None))
    }
  }

  pub fn to_import_map_path(&self) -> Option<String> {
    self.json.import_map.clone()
  }

  pub fn to_lint_config(&self) -> Result<Option<LintConfig>, AnyError> {
    if let Some(config) = self.json.lint.clone() {
      let lint_config: SerializedLintConfig = serde_json::from_value(config)
        .context("Failed to parse \"lint\" configuration")?;
      Ok(Some(lint_config.into_resolved(&self.specifier)?))
    } else {
      Ok(None)
    }
  }

  pub fn to_test_config(&self) -> Result<Option<TestConfig>, AnyError> {
    if let Some(config) = self.json.test.clone() {
      let lint_config: SerializedTestConfig = serde_json::from_value(config)
        .context("Failed to parse \"test\" configuration")?;
      Ok(Some(lint_config.into_resolved(&self.specifier)?))
    } else {
      Ok(None)
    }
  }

  /// Return any tasks that are defined in the configuration file as a sequence
  /// of JSON objects providing the name of the task and the arguments of the
  /// task in a detail field.
  pub fn to_lsp_tasks(&self) -> Option<Value> {
    let value = self.json.tasks.clone()?;
    let tasks: BTreeMap<String, String> = serde_json::from_value(value).ok()?;
    Some(
      tasks
        .into_iter()
        .map(|(key, value)| {
          json!({
            "name": key,
            "detail": value,
          })
        })
        .collect(),
    )
  }

  pub fn to_tasks_config(
    &self,
  ) -> Result<Option<BTreeMap<String, String>>, AnyError> {
    if let Some(config) = self.json.tasks.clone() {
      let tasks_config: BTreeMap<String, String> =
        serde_json::from_value(config)
          .context("Failed to parse \"tasks\" configuration")?;
      Ok(Some(tasks_config))
    } else {
      Ok(None)
    }
  }

  /// If the configuration file contains "extra" modules (like TypeScript
  /// `"types"`) options, return them as imports to be added to a module graph.
  pub fn to_maybe_imports(&self) -> MaybeImportsResult {
    let mut imports = Vec::new();
    let compiler_options_value =
      if let Some(value) = self.json.compiler_options.as_ref() {
        value
      } else {
        return Ok(None);
      };
    let compiler_options: CompilerOptions =
      serde_json::from_value(compiler_options_value.clone())?;
    if let Some(types) = compiler_options.types {
      imports.extend(types);
    }
    if compiler_options.jsx == Some("react-jsx".to_string()) {
      imports.push(format!(
        "{}/jsx-runtime",
        compiler_options.jsx_import_source.ok_or_else(|| custom_error("TypeError", "Compiler option 'jsx' set to 'react-jsx', but no 'jsxImportSource' defined."))?
      ));
    } else if compiler_options.jsx == Some("react-jsxdev".to_string()) {
      imports.push(format!(
        "{}/jsx-dev-runtime",
        compiler_options.jsx_import_source.ok_or_else(|| custom_error("TypeError", "Compiler option 'jsx' set to 'react-jsxdev', but no 'jsxImportSource' defined."))?
      ));
    }
    if !imports.is_empty() {
      let referrer = self.specifier.clone();
      Ok(Some(vec![(referrer, imports)]))
    } else {
      Ok(None)
    }
  }

  /// Based on the compiler options in the configuration file, return the
  /// implied JSX import source module.
  pub fn to_maybe_jsx_import_source_module(&self) -> Option<String> {
    let compiler_options_value = self.json.compiler_options.as_ref()?;
    let compiler_options: CompilerOptions =
      serde_json::from_value(compiler_options_value.clone()).ok()?;
    match compiler_options.jsx.as_deref() {
      Some("react-jsx") => Some("jsx-runtime".to_string()),
      Some("react-jsxdev") => Some("jsx-dev-runtime".to_string()),
      _ => None,
    }
  }

  pub fn to_fmt_config(&self) -> Result<Option<FmtConfig>, AnyError> {
    if let Some(config) = self.json.fmt.clone() {
      let fmt_config: SerializedFmtConfig = serde_json::from_value(config)
        .context("Failed to parse \"fmt\" configuration")?;
      Ok(Some(fmt_config.into_resolved(&self.specifier)?))
    } else {
      Ok(None)
    }
  }

  pub fn resolve_tasks_config(
    &self,
  ) -> Result<BTreeMap<String, String>, AnyError> {
    let maybe_tasks_config = self.to_tasks_config()?;
    if let Some(tasks_config) = maybe_tasks_config {
      for key in tasks_config.keys() {
        if key.is_empty() {
          bail!("Configuration file task names cannot be empty");
        } else if !key
          .chars()
          .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':'))
        {
          bail!("Configuration file task names must only contain alpha-numeric characters, colons (:), underscores (_), or dashes (-). Task: {}", key);
        } else if !key.chars().next().unwrap().is_ascii_alphabetic() {
          bail!("Configuration file task names must start with an alphabetic character. Task: {}", key);
        }
      }
      Ok(tasks_config)
    } else {
      bail!("No tasks found in configuration file")
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::serde_json::json;
  use pretty_assertions::assert_eq;

  #[test]
  fn read_config_file_relative() {
    let config_file =
      ConfigFile::read("tests/testdata/module_graph/tsconfig.json")
        .expect("Failed to load config file");
    assert!(config_file.json.compiler_options.is_some());
  }

  #[test]
  fn read_config_file_absolute() {
    let path = test_util::testdata_path().join("module_graph/tsconfig.json");
    let config_file = ConfigFile::read(path.to_str().unwrap())
      .expect("Failed to load config file");
    assert!(config_file.json.compiler_options.is_some());
  }

  #[test]
  fn include_config_path_on_error() {
    let error = ConfigFile::read("404.json").err().unwrap();
    assert!(error.to_string().contains("404.json"));
  }

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
      },
      "lint": {
        "files": {
          "include": ["src/"],
          "exclude": ["src/testdata/"]
        },
        "rules": {
          "tags": ["recommended"],
          "include": ["ban-untagged-todo"]
        }
      },
      "fmt": {
        "files": {
          "include": ["src/"],
          "exclude": ["src/testdata/"]
        },
        "options": {
          "useTabs": true,
          "lineWidth": 80,
          "indentWidth": 4,
          "singleQuote": true,
          "proseWrap": "preserve"
        }
      },
      "tasks": {
        "build": "deno run --allow-read --allow-write build.ts",
        "server": "deno run --allow-net --allow-read server.ts"
      }
    }"#;
    let config_dir = ModuleSpecifier::parse("file:///deno/").unwrap();
    let config_specifier = config_dir.join("tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, &config_specifier).unwrap();
    let (options_value, ignored) =
      config_file.to_compiler_options().expect("error parsing");
    assert!(options_value.is_object());
    let options = options_value.as_object().unwrap();
    assert!(options.contains_key("strict"));
    assert_eq!(options.len(), 1);
    assert_eq!(
      ignored,
      Some(IgnoredCompilerOptions {
        items: vec!["build".to_string()],
        maybe_specifier: Some(config_specifier),
      }),
    );

    let lint_config = config_file
      .to_lint_config()
      .expect("error parsing lint object")
      .expect("lint object should be defined");
    assert_eq!(
      lint_config.files.include,
      vec![config_dir.join("src/").unwrap()]
    );
    assert_eq!(
      lint_config.files.exclude,
      vec![config_dir.join("src/testdata/").unwrap()]
    );
    assert_eq!(
      lint_config.rules.include,
      Some(vec!["ban-untagged-todo".to_string()])
    );
    assert_eq!(
      lint_config.rules.tags,
      Some(vec!["recommended".to_string()])
    );
    assert!(lint_config.rules.exclude.is_none());

    let fmt_config = config_file
      .to_fmt_config()
      .expect("error parsing fmt object")
      .expect("fmt object should be defined");
    assert_eq!(
      fmt_config.files.include,
      vec![config_dir.join("src/").unwrap()]
    );
    assert_eq!(
      fmt_config.files.exclude,
      vec![config_dir.join("src/testdata/").unwrap()]
    );
    assert_eq!(fmt_config.options.use_tabs, Some(true));
    assert_eq!(fmt_config.options.line_width, Some(80));
    assert_eq!(fmt_config.options.indent_width, Some(4));
    assert_eq!(fmt_config.options.single_quote, Some(true));

    let tasks_config = config_file.to_tasks_config().unwrap().unwrap();
    assert_eq!(
      tasks_config["build"],
      "deno run --allow-read --allow-write build.ts",
    );
    assert_eq!(
      tasks_config["server"],
      "deno run --allow-net --allow-read server.ts"
    );
  }

  #[test]
  fn test_parse_config_with_empty_file() {
    let config_text = "";
    let config_specifier =
      ModuleSpecifier::parse("file:///deno/tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, &config_specifier).unwrap();
    let (options_value, _) =
      config_file.to_compiler_options().expect("error parsing");
    assert!(options_value.is_object());
  }

  #[test]
  fn test_parse_config_with_commented_file() {
    let config_text = r#"//{"foo":"bar"}"#;
    let config_specifier =
      ModuleSpecifier::parse("file:///deno/tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, &config_specifier).unwrap();
    let (options_value, _) =
      config_file.to_compiler_options().expect("error parsing");
    assert!(options_value.is_object());
  }

  #[test]
  fn test_parse_config_with_invalid_file() {
    let config_text = "{foo:bar}";
    let config_specifier =
      ModuleSpecifier::parse("file:///deno/tsconfig.json").unwrap();
    // Emit error: Unable to parse config file JSON "<config_path>" because of Unexpected token on line 1 column 6.
    assert!(ConfigFile::new(config_text, &config_specifier).is_err());
  }

  #[test]
  fn test_parse_config_with_not_object_file() {
    let config_text = "[]";
    let config_specifier =
      ModuleSpecifier::parse("file:///deno/tsconfig.json").unwrap();
    // Emit error: config file JSON "<config_path>" should be an object
    assert!(ConfigFile::new(config_text, &config_specifier).is_err());
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

  #[test]
  fn discover_from_success() {
    // testdata/fmt/deno.jsonc exists
    let testdata = test_util::testdata_path();
    let c_md = testdata.join("fmt/with_config/subdir/c.md");
    let mut checked = HashSet::new();
    let config_file = ConfigFile::discover_from(&c_md, &mut checked)
      .unwrap()
      .unwrap();
    assert!(checked.contains(c_md.parent().unwrap()));
    assert!(!checked.contains(&testdata));
    let fmt_config = config_file.to_fmt_config().unwrap().unwrap();
    let expected_exclude = ModuleSpecifier::from_file_path(
      testdata.join("fmt/with_config/subdir/b.ts"),
    )
    .unwrap();
    assert_eq!(fmt_config.files.exclude, vec![expected_exclude]);

    // Now add all ancestors of testdata to checked.
    for a in testdata.ancestors() {
      checked.insert(a.to_path_buf());
    }

    // If we call discover_from again starting at testdata, we ought to get None.
    assert!(ConfigFile::discover_from(&testdata, &mut checked)
      .unwrap()
      .is_none());
  }

  #[test]
  fn discover_from_malformed() {
    let testdata = test_util::testdata_path();
    let d = testdata.join("malformed_config/");
    let mut checked = HashSet::new();
    let err = ConfigFile::discover_from(&d, &mut checked).unwrap_err();
    assert!(err.to_string().contains("Unable to parse config file"));
  }

  #[test]
  fn tasks_no_tasks() {
    run_task_error_test(r#"{}"#, "No tasks found in configuration file");
  }

  #[test]
  fn task_name_invalid_chars() {
    run_task_error_test(
      r#"{
        "tasks": {
          "build": "deno test",
          "some%test": "deno bundle mod.ts"
        }
      }"#,
      concat!(
        "Configuration file task names must only contain alpha-numeric ",
        "characters, colons (:), underscores (_), or dashes (-). Task: some%test",
      ),
    );
  }

  #[test]
  fn task_name_non_alpha_starting_char() {
    run_task_error_test(
      r#"{
        "tasks": {
          "build": "deno test",
          "1test": "deno bundle mod.ts"
        }
      }"#,
      concat!(
        "Configuration file task names must start with an ",
        "alphabetic character. Task: 1test",
      ),
    );
  }

  #[test]
  fn task_name_empty() {
    run_task_error_test(
      r#"{
        "tasks": {
          "build": "deno test",
          "": "deno bundle mod.ts"
        }
      }"#,
      "Configuration file task names cannot be empty",
    );
  }

  fn run_task_error_test(config_text: &str, expected_error: &str) {
    let config_dir = ModuleSpecifier::parse("file:///deno/").unwrap();
    let config_specifier = config_dir.join("tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, &config_specifier).unwrap();
    assert_eq!(
      config_file
        .resolve_tasks_config()
        .err()
        .unwrap()
        .to_string(),
      expected_error,
    );
  }
}
