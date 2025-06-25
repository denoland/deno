// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use boxed_error::Boxed;
use deno_error::JsError;
use deno_path_util::url_from_file_path;
use deno_path_util::url_parent;
use deno_path_util::url_to_file_path;
use import_map::ImportMapWithDiagnostics;
use indexmap::IndexMap;
use jsonc_parser::ParseResult;
use serde::de;
use serde::de::Unexpected;
use serde::de::Visitor;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde_json::json;
use serde_json::Value;
use sys_traits::FsRead;
use thiserror::Error;
use ts::parse_compiler_options;
use url::Url;

use crate::glob::FilePatterns;
use crate::glob::PathOrPatternSet;
use crate::util::is_skippable_io_error;
use crate::UrlToFilePathError;

mod ts;

pub use ts::CompilerOptions;
pub use ts::EmitConfigOptions;
pub use ts::IgnoredCompilerOptions;
pub use ts::ParsedCompilerOptions;
pub use ts::RawJsxCompilerOptions;

#[derive(Clone, Debug, Default, Deserialize, Hash, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LintRulesConfig {
  pub tags: Option<Vec<String>>,
  pub include: Option<Vec<String>>,
  pub exclude: Option<Vec<String>>,
}

#[derive(Debug, JsError, Boxed)]
pub struct IntoResolvedError(pub Box<IntoResolvedErrorKind>);

#[derive(Debug, Error, JsError)]
pub enum IntoResolvedErrorKind {
  #[class(uri)]
  #[error(transparent)]
  UrlParse(#[from] url::ParseError),
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePath(#[from] UrlToFilePathError),
  #[class(inherit)]
  #[error("Invalid include: {0}")]
  InvalidInclude(crate::glob::PathOrPatternParseError),
  #[class(inherit)]
  #[error("Invalid exclude: {0}")]
  InvalidExclude(crate::glob::FromExcludeRelativePathOrPatternsError),
}

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Failed deserilaizing \"compilerOptions\".\"types\" in {}", self.specifier)]
pub struct CompilerOptionTypesDeserializeError {
  specifier: Url,
  #[source]
  source: serde_json::Error,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct SerializedFilesConfig {
  pub include: Option<Vec<String>>,
  pub exclude: Vec<String>,
}

impl SerializedFilesConfig {
  pub fn into_resolved(
    self,
    config_file_specifier: &Url,
  ) -> Result<FilePatterns, IntoResolvedError> {
    let config_dir = url_to_file_path(&url_parent(config_file_specifier))?;
    Ok(FilePatterns {
      base: config_dir.clone(),
      include: match self.include {
        Some(i) => Some(
          PathOrPatternSet::from_include_relative_path_or_patterns(
            &config_dir,
            &i,
          )
          .map_err(IntoResolvedErrorKind::InvalidInclude)?,
        ),
        None => None,
      },
      exclude: PathOrPatternSet::from_exclude_relative_path_or_patterns(
        &config_dir,
        &self.exclude,
      )
      .map_err(IntoResolvedErrorKind::InvalidExclude)?,
    })
  }
}

/// `lint` config representation for serde
///
/// fields `include` and `exclude` are expanded from [SerializedFilesConfig].
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct SerializedLintConfig {
  pub rules: LintRulesConfig,
  pub include: Option<Vec<String>>,
  pub exclude: Vec<String>,

  #[serde(rename = "files")]
  pub deprecated_files: serde_json::Value,
  pub report: Option<String>,
  pub plugins: Vec<String>,
}

impl SerializedLintConfig {
  pub fn into_resolved(
    self,
    config_file_specifier: &Url,
  ) -> Result<LintConfig, IntoResolvedError> {
    let (include, exclude) = (self.include, self.exclude);
    let files = SerializedFilesConfig { include, exclude };
    if !self.deprecated_files.is_null() {
      log::warn!( "Warning: \"files\" configuration in \"lint\" was removed in Deno 2, use \"include\" and \"exclude\" instead.");
    }
    Ok(LintConfig {
      options: LintOptionsConfig {
        rules: self.rules,
        plugins: self
          .plugins
          .into_iter()
          .map(|specifier| LintPluginConfig {
            specifier,
            base: config_file_specifier.clone(),
          })
          .collect(),
      },
      files: files.into_resolved(config_file_specifier)?,
    })
  }
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct LintPluginConfig {
  pub specifier: String,
  pub base: Url,
}

#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct LintOptionsConfig {
  pub rules: LintRulesConfig,
  pub plugins: Vec<LintPluginConfig>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct LintConfig {
  pub options: LintOptionsConfig,
  pub files: FilePatterns,
}

impl LintConfig {
  pub fn new_with_base(base: PathBuf) -> Self {
    // note: don't create Default implementations of these
    // config structs because the base of FilePatterns matters
    Self {
      options: Default::default(),
      files: FilePatterns::new_with_base(base),
    }
  }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum ProseWrap {
  Always,
  Never,
  Preserve,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum QuoteProps {
  AsNeeded,
  Consistent,
  Preserve,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum NewLineKind {
  Auto,
  #[serde(rename = "lf")]
  LineFeed,
  #[serde(rename = "crlf")]
  CarriageReturnLineFeed,
  System,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum UseBraces {
  Maintain,
  WhenNotSingleLine,
  Always,
  PreferNone,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum BracePosition {
  Maintain,
  SameLine,
  NextLine,
  SameLineUnlessHanging,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum SingleBodyPosition {
  Maintain,
  SameLine,
  NextLine,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum NextControlFlowPosition {
  Maintain,
  SameLine,
  NextLine,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum TrailingCommas {
  Always,
  Never,
  OnlyMultiLine,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum OperatorPosition {
  Maintain,
  SameLine,
  NextLine,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum BracketPosition {
  Maintain,
  SameLine,
  NextLine,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum MultiLineParens {
  Never,
  Prefer,
  Always,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub enum SeparatorKind {
  SemiColon,
  Comma,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Hash, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct FmtOptionsConfig {
  pub use_tabs: Option<bool>,
  pub line_width: Option<u32>,
  pub indent_width: Option<u8>,
  pub single_quote: Option<bool>,
  pub prose_wrap: Option<ProseWrap>,
  pub semi_colons: Option<bool>,
  pub quote_props: Option<QuoteProps>,
  pub new_line_kind: Option<NewLineKind>,
  pub use_braces: Option<UseBraces>,
  pub brace_position: Option<BracePosition>,
  pub single_body_position: Option<SingleBodyPosition>,
  pub next_control_flow_position: Option<NextControlFlowPosition>,
  pub trailing_commas: Option<TrailingCommas>,
  pub operator_position: Option<OperatorPosition>,
  pub jsx_bracket_position: Option<BracketPosition>,
  pub jsx_force_new_lines_surrounding_content: Option<bool>,
  pub jsx_multi_line_parens: Option<MultiLineParens>,
  pub type_literal_separator_kind: Option<SeparatorKind>,
  pub space_around: Option<bool>,
  pub space_surrounding_properties: Option<bool>,
}

impl FmtOptionsConfig {
  pub fn is_empty(&self) -> bool {
    self.use_tabs.is_none()
      && self.line_width.is_none()
      && self.indent_width.is_none()
      && self.single_quote.is_none()
      && self.prose_wrap.is_none()
      && self.semi_colons.is_none()
      && self.quote_props.is_none()
      && self.new_line_kind.is_none()
      && self.use_braces.is_none()
      && self.brace_position.is_none()
      && self.single_body_position.is_none()
      && self.next_control_flow_position.is_none()
      && self.trailing_commas.is_none()
      && self.operator_position.is_none()
      && self.jsx_bracket_position.is_none()
      && self.jsx_force_new_lines_surrounding_content.is_none()
      && self.jsx_multi_line_parens.is_none()
      && self.type_literal_separator_kind.is_none()
      && self.space_around.is_none()
      && self.space_surrounding_properties.is_none()
  }
}

/// Choose between flat and nested fmt options.
///
/// `options` has precedence over `deprecated_options`.
/// when `deprecated_options` is present, a warning is logged.
///
/// caveat: due to default values, it's not possible to distinguish between
/// an empty configuration and a configuration with default values.
/// `{ "fmt": {} } is equivalent to `{ "fmt": { "options": {} } }`
/// and it wouldn't be able to emit warning for `{ "fmt": { "options": {}, "semiColons": "false" } }`.
///
/// # Arguments
///
/// * `options` - Flat options.
/// * `deprecated_options` - Nested files configuration ("option").
fn choose_fmt_options(
  options: FmtOptionsConfig,
  deprecated_options: FmtOptionsConfig,
) -> FmtOptionsConfig {
  const DEPRECATED_OPTIONS: &str =
    "Warning: \"options\" configuration is deprecated";
  const FLAT_OPTION: &str = "\"flat\" options";

  let (options_nonempty, deprecated_options_nonempty) =
    (!options.is_empty(), !deprecated_options.is_empty());

  match (options_nonempty, deprecated_options_nonempty) {
    (true, true) => {
      log::warn!("{DEPRECATED_OPTIONS} and ignored by {FLAT_OPTION}.");
      options
    }
    (true, false) => options,
    (false, true) => {
      log::warn!("{DEPRECATED_OPTIONS}. Please use {FLAT_OPTION} instead.");
      deprecated_options
    }
    (false, false) => FmtOptionsConfig::default(),
  }
}

/// `fmt` config representation for serde
///
/// fields from `use_tabs`..`semi_colons` are expanded from [FmtOptionsConfig].
/// fields `include` and `exclude` are expanded from [SerializedFilesConfig].
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
struct SerializedFmtConfig {
  pub use_tabs: Option<bool>,
  pub line_width: Option<u32>,
  pub indent_width: Option<u8>,
  pub single_quote: Option<bool>,
  pub prose_wrap: Option<ProseWrap>,
  pub semi_colons: Option<bool>,
  pub quote_props: Option<QuoteProps>,
  pub new_line_kind: Option<NewLineKind>,
  pub use_braces: Option<UseBraces>,
  pub brace_position: Option<BracePosition>,
  pub single_body_position: Option<SingleBodyPosition>,
  pub next_control_flow_position: Option<NextControlFlowPosition>,
  pub trailing_commas: Option<TrailingCommas>,
  pub operator_position: Option<OperatorPosition>,
  #[serde(rename = "jsx.bracketPosition")]
  pub jsx_bracket_position: Option<BracketPosition>,
  #[serde(rename = "jsx.forceNewLinesSurroundingContent")]
  pub jsx_force_new_lines_surrounding_content: Option<bool>,
  #[serde(rename = "jsx.multiLineParens")]
  pub jsx_multi_line_parens: Option<MultiLineParens>,
  #[serde(rename = "typeLiteral.separatorKind")]
  pub type_literal_separator_kind: Option<SeparatorKind>,
  pub space_around: Option<bool>,
  pub space_surrounding_properties: Option<bool>,
  #[serde(rename = "options")]
  pub deprecated_options: FmtOptionsConfig,
  pub include: Option<Vec<String>>,
  pub exclude: Vec<String>,
  #[serde(rename = "files")]
  pub deprecated_files: serde_json::Value,
}

impl SerializedFmtConfig {
  pub fn into_resolved(
    self,
    config_file_specifier: &Url,
  ) -> Result<FmtConfig, IntoResolvedError> {
    let (include, exclude) = (self.include, self.exclude);
    let files = SerializedFilesConfig { include, exclude };
    let options = FmtOptionsConfig {
      use_tabs: self.use_tabs,
      line_width: self.line_width,
      indent_width: self.indent_width,
      single_quote: self.single_quote,
      prose_wrap: self.prose_wrap,
      semi_colons: self.semi_colons,
      quote_props: self.quote_props,
      new_line_kind: self.new_line_kind,
      use_braces: self.use_braces,
      brace_position: self.brace_position,
      single_body_position: self.single_body_position,
      next_control_flow_position: self.next_control_flow_position,
      trailing_commas: self.trailing_commas,
      operator_position: self.operator_position,
      jsx_bracket_position: self.jsx_bracket_position,
      jsx_force_new_lines_surrounding_content: self
        .jsx_force_new_lines_surrounding_content,
      jsx_multi_line_parens: self.jsx_multi_line_parens,
      type_literal_separator_kind: self.type_literal_separator_kind,
      space_around: self.space_around,
      space_surrounding_properties: self.space_surrounding_properties,
    };
    if !self.deprecated_files.is_null() {
      log::warn!( "Warning: \"files\" configuration in \"fmt\" was removed in Deno 2, use \"include\" and \"exclude\" instead.");
    }
    Ok(FmtConfig {
      options: choose_fmt_options(options, self.deprecated_options),
      files: files.into_resolved(config_file_specifier)?,
    })
  }
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct FmtConfig {
  pub options: FmtOptionsConfig,
  pub files: FilePatterns,
}

impl FmtConfig {
  pub fn new_with_base(base: PathBuf) -> Self {
    Self {
      options: Default::default(),
      files: FilePatterns::new_with_base(base),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportsConfig {
  base: Url,
  map: IndexMap<String, String>,
}

impl ExportsConfig {
  pub fn into_map(self) -> IndexMap<String, String> {
    self.map
  }

  pub fn get(&self, export_name: &str) -> Option<&String> {
    self.map.get(export_name)
  }

  pub fn get_resolved(
    &self,
    export_name: &str,
  ) -> Result<Option<Url>, url::ParseError> {
    match self.get(export_name) {
      Some(name) => self.base.join(name).map(Some),
      None => Ok(None),
    }
  }
}

/// `test` config representation for serde
///
/// fields `include` and `exclude` are expanded from [SerializedFilesConfig].
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct SerializedTestConfig {
  pub include: Option<Vec<String>>,
  pub exclude: Vec<String>,
  #[serde(rename = "files")]
  pub deprecated_files: serde_json::Value,
}

impl SerializedTestConfig {
  pub fn into_resolved(
    self,
    config_file_specifier: &Url,
  ) -> Result<TestConfig, IntoResolvedError> {
    let (include, exclude) = (self.include, self.exclude);
    let files = SerializedFilesConfig { include, exclude };
    if !self.deprecated_files.is_null() {
      log::warn!( "Warning: \"files\" configuration in \"test\" was removed in Deno 2, use \"include\" and \"exclude\" instead.");
    }
    Ok(TestConfig {
      files: files.into_resolved(config_file_specifier)?,
    })
  }
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct TestConfig {
  pub files: FilePatterns,
}

impl TestConfig {
  pub fn new_with_base(base: PathBuf) -> Self {
    Self {
      files: FilePatterns::new_with_base(base),
    }
  }
}

/// `publish` config representation for serde
///
/// fields `include` and `exclude` are expanded from [SerializedFilesConfig].
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct SerializedPublishConfig {
  pub include: Option<Vec<String>>,
  pub exclude: Vec<String>,
}

impl SerializedPublishConfig {
  pub fn into_resolved(
    self,
    config_file_specifier: &Url,
  ) -> Result<PublishConfig, IntoResolvedError> {
    let (include, exclude) = (self.include, self.exclude);
    let files = SerializedFilesConfig { include, exclude };

    Ok(PublishConfig {
      files: files.into_resolved(config_file_specifier)?,
    })
  }
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PublishConfig {
  pub files: FilePatterns,
}

impl PublishConfig {
  pub fn new_with_base(base: PathBuf) -> Self {
    Self {
      files: FilePatterns::new_with_base(base),
    }
  }
}

/// `bench` config representation for serde
///
/// fields `include` and `exclude` are expanded from [SerializedFilesConfig].
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct SerializedBenchConfig {
  pub include: Option<Vec<String>>,
  pub exclude: Vec<String>,
  #[serde(rename = "files")]
  pub deprecated_files: serde_json::Value,
}

impl SerializedBenchConfig {
  pub fn into_resolved(
    self,
    config_file_specifier: &Url,
  ) -> Result<BenchConfig, IntoResolvedError> {
    let (include, exclude) = (self.include, self.exclude);
    let files = SerializedFilesConfig { include, exclude };
    if !self.deprecated_files.is_null() {
      log::warn!( "Warning: \"files\" configuration in \"bench\" was removed in Deno 2, use \"include\" and \"exclude\" instead.");
    }
    Ok(BenchConfig {
      files: files.into_resolved(config_file_specifier)?,
    })
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BenchConfig {
  pub files: FilePatterns,
}

impl BenchConfig {
  pub fn new_with_base(base: PathBuf) -> Self {
    Self {
      files: FilePatterns::new_with_base(base),
    }
  }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum LockConfig {
  Bool(bool),
  PathBuf(PathBuf),
  Object {
    path: Option<PathBuf>,
    frozen: Option<bool>,
  },
}

impl LockConfig {
  pub fn frozen(&self) -> bool {
    matches!(
      self,
      LockConfig::Object {
        frozen: Some(true),
        ..
      }
    )
  }
}

#[derive(Debug, Error, JsError)]
#[class(inherit)]
#[error("Failed to parse \"workspace\" configuration.")]
pub struct WorkspaceConfigParseError(#[source] serde_json::Error);

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
  pub members: Vec<String>,
}

#[derive(Debug, Error, JsError)]
#[class(inherit)]
#[error("Failed to parse \"link\" configuration.")]
pub struct LinkConfigParseError(#[source] serde_json::Error);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TaskDefinition {
  pub command: Option<String>,
  #[serde(default)]
  pub dependencies: Vec<String>,
  #[serde(default)]
  pub description: Option<String>,
}

#[cfg(test)]
impl From<&str> for TaskDefinition {
  fn from(value: &str) -> Self {
    Self {
      command: Some(value.to_string()),
      dependencies: vec![],
      description: None,
    }
  }
}

impl TaskDefinition {
  pub fn deserialize_tasks<'de, D>(
    deserializer: D,
  ) -> Result<IndexMap<String, TaskDefinition>, D::Error>
  where
    D: Deserializer<'de>,
  {
    use std::fmt;

    use serde::de::MapAccess;
    use serde::de::Visitor;

    struct TasksVisitor;

    impl<'de> Visitor<'de> for TasksVisitor {
      type Value = IndexMap<String, TaskDefinition>;

      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map of task definitions")
      }

      fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
      where
        M: MapAccess<'de>,
      {
        let mut map = IndexMap::with_capacity(access.size_hint().unwrap_or(4));

        while let Some((key, value)) =
          access.next_entry::<String, serde_json::Value>()?
        {
          let task_def = match value {
            serde_json::Value::String(command) => TaskDefinition {
              command: Some(command),
              dependencies: Vec::new(),
              description: None,
            },
            serde_json::Value::Object(_) => {
              serde_json::from_value(value).map_err(serde::de::Error::custom)?
            }
            _ => {
              return Err(serde::de::Error::custom("invalid task definition"))
            }
          };
          map.insert(key, task_def);
        }

        Ok(map)
      }
    }

    deserializer.deserialize_map(TasksVisitor)
  }
}

#[derive(Debug, JsError, Boxed)]
pub struct ConfigFileReadError(pub Box<ConfigFileReadErrorKind>);

impl ConfigFileReadError {
  pub fn is_not_found(&self) -> bool {
    if let ConfigFileReadErrorKind::FailedReading { source: ioerr, .. } =
      self.as_kind()
    {
      matches!(ioerr.kind(), std::io::ErrorKind::NotFound)
    } else {
      false
    }
  }
}

#[derive(Debug, Error, JsError)]
pub enum ConfigFileReadErrorKind {
  #[class(type)]
  #[error("Could not convert config file path to specifier. Path: {0}")]
  PathToUrl(PathBuf),
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePathError(#[from] UrlToFilePathError),
  #[class(inherit)]
  #[error("Error reading config file '{specifier}'.")]
  FailedReading {
    specifier: Url,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(type)]
  #[error("Unable to parse config file JSON {specifier}.")]
  Parse {
    specifier: Url,
    #[source]
    source: Box<jsonc_parser::errors::ParseError>,
  },
  #[class(inherit)]
  #[error("Failed deserializing config file '{specifier}'.")]
  Deserialize {
    specifier: Url,
    #[source]
    #[inherit]
    source: serde_json::Error,
  },
  #[class(type)]
  #[error("Config file JSON should be an object '{specifier}'.")]
  NotObject { specifier: Url },
}

#[derive(Debug, Error, JsError)]
#[class(type)]
#[error("Unsupported \"nodeModulesDir\" value.")]
pub struct NodeModulesDirParseError {
  #[source]
  pub source: serde_json::Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeModulesDirMode {
  Auto,
  Manual,
  None,
}

impl<'de> Deserialize<'de> for NodeModulesDirMode {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct NodeModulesDirModeVisitor;

    impl Visitor<'_> for NodeModulesDirModeVisitor {
      type Value = NodeModulesDirMode;

      fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter,
      ) -> std::fmt::Result {
        formatter.write_str(r#""auto", "manual", or "none""#)
      }

      fn visit_str<E>(self, value: &str) -> Result<NodeModulesDirMode, E>
      where
        E: de::Error,
      {
        match value {
          "auto" => Ok(NodeModulesDirMode::Auto),
          "manual" => Ok(NodeModulesDirMode::Manual),
          "none" => Ok(NodeModulesDirMode::None),
          _ => Err(de::Error::invalid_value(Unexpected::Str(value), &self)),
        }
      }

      fn visit_bool<E>(self, value: bool) -> Result<NodeModulesDirMode, E>
      where
        E: de::Error,
      {
        if value {
          Ok(NodeModulesDirMode::Auto)
        } else {
          Ok(NodeModulesDirMode::None)
        }
      }
    }

    deserializer.deserialize_any(NodeModulesDirModeVisitor)
  }
}

impl std::fmt::Display for NodeModulesDirMode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

impl NodeModulesDirMode {
  pub fn as_str(self) -> &'static str {
    match self {
      NodeModulesDirMode::Auto => "auto",
      NodeModulesDirMode::Manual => "manual",
      NodeModulesDirMode::None => "none",
    }
  }

  pub fn uses_node_modules_dir(self) -> bool {
    matches!(self, Self::Manual | Self::Auto)
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFileJson {
  pub compiler_options: Option<Value>,
  pub import_map: Option<String>,
  pub imports: Option<Value>,
  pub scopes: Option<Value>,
  pub lint: Option<Value>,
  pub fmt: Option<Value>,
  pub tasks: Option<Value>,
  pub test: Option<Value>,
  pub bench: Option<Value>,
  pub lock: Option<Value>,
  pub exclude: Option<Value>,
  pub node_modules_dir: Option<Value>,
  pub vendor: Option<bool>,
  pub license: Option<Value>,
  pub publish: Option<Value>,

  pub name: Option<String>,
  pub version: Option<String>,
  pub workspace: Option<Value>,
  pub links: Option<Value>,
  #[serde(rename = "patch")]
  pub(crate) deprecated_patch: Option<Value>,
  #[serde(rename = "workspaces")]
  pub(crate) deprecated_workspaces: Option<Vec<String>>,
  pub exports: Option<Value>,
  #[serde(default)]
  pub unstable: Vec<String>,
}

pub trait DenoJsonCache {
  fn get(&self, path: &Path) -> Option<ConfigFileRc>;
  fn set(&self, path: PathBuf, deno_json: ConfigFileRc);
}

#[derive(Debug, Error, JsError)]
#[class(type)]
#[error("compilerOptions should be an object at '{specifier}'")]
pub struct CompilerOptionsParseError {
  pub specifier: Url,
  #[source]
  pub source: serde_json::Error,
}

#[derive(Debug, Error, JsError)]
pub enum ConfigFileError {
  #[class(inherit)]
  #[error(transparent)]
  CompilerOptionsParseError(CompilerOptionsParseError),
  #[class(type)]
  #[error("Only file: specifiers are supported for security reasons in import maps stored in a deno.json. To use a remote import map, use the --import-map flag and \"deno.importMap\" in the language server config")]
  OnlyFileSpecifiersSupported,
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePath(#[from] UrlToFilePathError),
  #[class(inherit)]
  #[error(transparent)]
  UrlParse(#[from] url::ParseError),
  #[class(inherit)]
  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),
  #[class(inherit)]
  #[error(transparent)]
  ImportMap(#[from] import_map::ImportMapError),
  #[class(inherit)]
  #[error(transparent)]
  Io(std::io::Error),
}

#[derive(Debug, Error, JsError)]
pub enum ConfigFileExportsError {
  #[class(type)]
  #[error("The {0} must not be empty. Use '.' if you meant the root export.")]
  KeyMustNotBeEmpty(Cow<'static, str>),
  #[class(type)]
  #[error("The {key} must start with './'. Did you mean '{suggestion}'?")]
  KeyMustStartWithDotSlash {
    key: Cow<'static, str>,
    suggestion: String,
  },
  #[class(type)]
  #[error("The {key} must not end with '/'. Did you mean '{suggestion}'?")]
  KeyMustNotEndWithSlash {
    key: Cow<'static, str>,
    suggestion: String,
  },
  #[class(type)]
  #[error("The {0} must only contain alphanumeric characters, underscores (_), dashes (-), dots (.), and slashes (/).")]
  KeyInvalidCharacter(Cow<'static, str>),
  #[class(type)]
  #[error("The {0} must not contain double slashes (//), or parts consisting entirely of dots (.).")]
  KeyTooManySlashesOrDots(Cow<'static, str>),
  #[class(type)]
  #[error("The path for the {0} must not be empty.")]
  ValueMustNotBeEmpty(Cow<'static, str>),
  #[class(type)]
  #[error("The path '{value}' at the {key} could not be resolved as a relative path from the config file. Did you mean '{suggestion}'?")]
  ValueCouldNotBeResolved {
    value: String,
    key: Cow<'static, str>,
    suggestion: String,
  },
  #[class(type)]
  #[error("The path '{value}' at the {key} must not end with '/'. Did you mean '{suggestion}'?")]
  ValueMustNotEndWithSlash {
    value: String,
    key: Cow<'static, str>,
    suggestion: String,
  },
  #[class(type)]
  #[error("The path '{value}' at the {key} is missing a file extension. Add a file extension such as '.js' or '.ts'.")]
  ValueMissingFileExtension {
    value: String,
    key: Cow<'static, str>,
  },
  #[class(type)]
  #[error("The path of the {key} must be a string, found invalid value '{value}'. Exports in deno.json do not support conditional exports.")]
  InvalidValueConditionalExports {
    key: Cow<'static, str>,
    value: Value,
  },
  #[class(type)]
  #[error(
    "The path of the {key} must be a string, found invalid value '{value}'."
  )]
  InvalidValue {
    key: Cow<'static, str>,
    value: Value,
  },
  #[class(type)]
  #[error(
    "The 'exports' key must be a string or object, found invalid value '{0}'."
  )]
  ExportsKeyInvalidValue(Value),
}

#[derive(Debug, Error, JsError)]
pub enum ToInvalidConfigError {
  #[class(inherit)]
  #[error("Invalid {config} config")]
  InvalidConfig {
    config: &'static str,
    #[source]
    #[inherit]
    source: IntoResolvedError,
  },
  #[class(inherit)]
  #[error("Failed to parse \"{config}\" configuration")]
  Parse {
    config: &'static str,
    #[source]
    #[inherit]
    source: serde_json::Error,
  },
}

#[derive(Debug, Error, JsError)]
#[class(type)]
pub enum ResolveTaskConfigError {
  #[error("Configuration file task names cannot be empty")]
  TaskNameEmpty,
  #[error("Configuration file task names must only contain alpha-numeric characters, colons (:), underscores (_), or dashes (-). Task: {0}")]
  TaskNameInvalidCharacter(String),
  #[error("Configuration file task names must start with an alphabetic character. Task: {0}")]
  TaskNameInvalidStartingCharacter(String),
  #[class(inherit)]
  #[error(transparent)]
  ToInvalidConfig(#[from] ToInvalidConfigError),
}

#[derive(Debug, Error, JsError)]
pub enum ResolveExportValueUrlsError {
  #[class(inherit)]
  #[error("Failed to parse exports at {specifier}")]
  ExportsConfig {
    specifier: Url,
    #[source]
    #[inherit]
    error: Box<ConfigFileExportsError>,
  },
  #[class(inherit)]
  #[error("Failed to join {specifier} with {value}")]
  JoinError {
    specifier: Url,
    value: String,
    #[source]
    #[inherit]
    error: url::ParseError,
  },
}

#[derive(Debug, Error, JsError)]
pub enum ToLockConfigError {
  #[class(inherit)]
  #[error(transparent)]
  ToInvalidConfigError(#[from] ToInvalidConfigError),
  #[class(inherit)]
  #[error(transparent)]
  UrlToFilePath(#[from] UrlToFilePathError),
}

#[allow(clippy::disallowed_types)]
pub type ConfigFileRc = crate::sync::MaybeArc<ConfigFile>;

#[derive(Clone, Debug)]
pub struct ConfigFile {
  pub specifier: Url,
  pub json: ConfigFileJson,
}

impl ConfigFile {
  /// Filenames that Deno will recognize when discovering config.
  pub(crate) fn resolve_config_file_names<'a>(
    additional_config_file_names: &[&'a str],
  ) -> Cow<'a, [&'a str]> {
    const CONFIG_FILE_NAMES: [&str; 2] = ["deno.json", "deno.jsonc"];
    if additional_config_file_names.is_empty() {
      Cow::Borrowed(&CONFIG_FILE_NAMES)
    } else {
      Cow::Owned(
        CONFIG_FILE_NAMES
          .iter()
          .copied()
          .chain(additional_config_file_names.iter().copied())
          .collect::<Vec<_>>(),
      )
    }
  }

  pub(crate) fn maybe_find_in_folder(
    sys: &impl FsRead,
    maybe_cache: Option<&dyn DenoJsonCache>,
    folder: &Path,
    config_file_names: &[&str],
  ) -> Result<Option<ConfigFileRc>, ConfigFileReadError> {
    fn is_skippable_err(e: &ConfigFileReadError) -> bool {
      if let ConfigFileReadErrorKind::FailedReading { source: ioerr, .. } =
        e.as_kind()
      {
        is_skippable_io_error(ioerr)
      } else {
        false
      }
    }

    for config_filename in config_file_names {
      let file_path = folder.join(config_filename);
      if let Some(item) = maybe_cache.and_then(|c| c.get(&file_path)) {
        return Ok(Some(item));
      }
      match ConfigFile::read(sys, &file_path) {
        Ok(cf) => {
          let cf = crate::sync::new_rc(cf);
          log::debug!("Config file found at '{}'", file_path.display());
          if let Some(cache) = maybe_cache {
            cache.set(file_path, cf.clone());
          }
          return Ok(Some(cf));
        }
        Err(e) if is_skippable_err(&e) => {
          // ok, keep going
        }
        Err(e) => {
          return Err(e);
        }
      }
    }
    Ok(None)
  }

  pub fn read(
    sys: &impl FsRead,
    config_path: &Path,
  ) -> Result<Self, ConfigFileReadError> {
    debug_assert!(config_path.is_absolute());
    let specifier = url_from_file_path(config_path).map_err(|_| {
      ConfigFileReadErrorKind::PathToUrl(config_path.to_path_buf()).into_box()
    })?;
    Self::from_specifier_and_path(sys, specifier, config_path)
  }

  pub fn from_specifier(
    sys: &impl FsRead,
    specifier: Url,
  ) -> Result<Self, ConfigFileReadError> {
    let config_path = url_to_file_path(&specifier)?;
    Self::from_specifier_and_path(sys, specifier, &config_path)
  }

  fn from_specifier_and_path(
    sys: &impl FsRead,
    specifier: Url,
    config_path: &Path,
  ) -> Result<Self, ConfigFileReadError> {
    let text = sys.fs_read_to_string_lossy(config_path).map_err(|err| {
      ConfigFileReadErrorKind::FailedReading {
        specifier: specifier.clone(),
        source: err,
      }
      .into_box()
    })?;
    Self::new(&text, specifier)
  }

  pub fn new(text: &str, specifier: Url) -> Result<Self, ConfigFileReadError> {
    let jsonc = match jsonc_parser::parse_to_ast(
      text,
      &Default::default(),
      &Default::default(),
    ) {
      Ok(ParseResult {
        value: Some(value @ jsonc_parser::ast::Value::Object(_)),
        ..
      }) => Value::from(value),
      Ok(ParseResult { value: None, .. }) => {
        json!({})
      }
      Err(e) => {
        return Err(
          ConfigFileReadErrorKind::Parse {
            specifier,
            source: Box::new(e),
          }
          .into_box(),
        );
      }
      _ => {
        return Err(
          ConfigFileReadErrorKind::NotObject { specifier }.into_box(),
        );
      }
    };
    let json: ConfigFileJson =
      serde_json::from_value(jsonc).map_err(|err| {
        ConfigFileReadErrorKind::Deserialize {
          specifier: specifier.clone(),
          source: err,
        }
        .into_box()
      })?;

    Ok(Self { specifier, json })
  }

  pub fn dir_path(&self) -> PathBuf {
    url_to_file_path(&self.specifier)
      .unwrap()
      .parent()
      .unwrap()
      .to_path_buf()
  }

  /// Returns if the configuration indicates that JavaScript should be
  /// type checked, otherwise None if not set.
  pub fn check_js(&self) -> Option<bool> {
    self
      .json
      .compiler_options
      .as_ref()
      .and_then(|co| co.get("checkJs").and_then(|v| v.as_bool()))
  }

  /// Parse `compilerOptions` and return a serde `Value`.
  /// The result also contains any options that were ignored.
  pub fn to_compiler_options(
    &self,
  ) -> Result<Option<ParsedCompilerOptions>, CompilerOptionsParseError> {
    let Some(compiler_options) = self.json.compiler_options.clone() else {
      return Ok(None);
    };
    let options: serde_json::Map<String, Value> =
      serde_json::from_value(compiler_options).map_err(|source| {
        CompilerOptionsParseError {
          specifier: self.specifier.clone(),
          source,
        }
      })?;
    Ok(Some(parse_compiler_options(options, Some(&self.specifier))))
  }

  pub fn to_import_map_specifier(
    &self,
  ) -> Result<Option<Url>, ConfigFileError> {
    let Some(value) = self.json.import_map.as_ref() else {
      return Ok(None);
    };
    // try to resolve as a url
    if let Ok(specifier) = Url::parse(value) {
      if specifier.scheme() != "file" {
        return Err(ConfigFileError::OnlyFileSpecifiersSupported);
      }
      return Ok(Some(specifier));
    }
    // now as a relative file path
    Ok(Some(url_parent(&self.specifier).join(value)?))
  }

  pub fn to_import_map_path(&self) -> Result<Option<PathBuf>, ConfigFileError> {
    let maybe_specifier = self.to_import_map_specifier()?;
    match maybe_specifier {
      Some(specifier) => Ok(Some(url_to_file_path(&specifier)?)),
      None => Ok(None),
    }
  }

  pub fn vendor(&self) -> Option<bool> {
    self.json.vendor
  }

  /// Resolves the import map potentially resolving the file specified
  /// at the "importMap" entry.
  pub fn to_import_map(
    &self,
    sys: &impl FsRead,
  ) -> Result<Option<ImportMapWithDiagnostics>, ConfigFileError> {
    let maybe_result = self.to_import_map_value(sys)?;
    match maybe_result {
      Some((specifier, value)) => {
        let import_map =
          import_map::parse_from_value(specifier.into_owned(), value)?;
        Ok(Some(import_map))
      }
      None => Ok(None),
    }
  }

  /// Resolves the import map `serde_json::Value` potentially resolving the
  /// file specified at the "importMap" entry.
  pub fn to_import_map_value(
    &self,
    sys: &impl FsRead,
  ) -> Result<Option<(Cow<Url>, serde_json::Value)>, ConfigFileError> {
    // has higher precedence over the path
    if self.json.imports.is_some() || self.json.scopes.is_some() {
      Ok(Some((
        Cow::Borrowed(&self.specifier),
        self.to_import_map_value_from_imports(),
      )))
    } else {
      let Some(specifier) = self.to_import_map_specifier()? else {
        return Ok(None);
      };
      let Ok(import_map_path) = url_to_file_path(&specifier) else {
        return Ok(None);
      };
      let text = sys
        .fs_read_to_string_lossy(&import_map_path)
        .map_err(ConfigFileError::Io)?;
      let value = serde_json::from_str(&text)?;
      // does not expand the imports because this one will use the import map standard
      Ok(Some((Cow::Owned(specifier), value)))
    }
  }

  /// Creates the import map from the imports entry.
  ///
  /// Warning: This does not take into account the 'importMap' entry. Use `to_import_map` instead.
  pub fn to_import_map_from_imports(
    &self,
  ) -> Result<ImportMapWithDiagnostics, ConfigFileError> {
    let value = self.to_import_map_value_from_imports();
    let result = import_map::parse_from_value(self.specifier.clone(), value)?;
    Ok(result)
  }

  pub fn to_import_map_value_from_imports(&self) -> Value {
    let mut value = serde_json::Map::with_capacity(2);
    if let Some(imports) = &self.json.imports {
      value.insert("imports".to_string(), imports.clone());
    }
    if let Some(scopes) = &self.json.scopes {
      value.insert("scopes".to_string(), scopes.clone());
    }
    import_map::ext::expand_import_map_value(Value::Object(value))
  }

  pub fn is_an_import_map(&self) -> bool {
    self.json.imports.is_some() || self.json.scopes.is_some()
  }

  pub fn is_package(&self) -> bool {
    self.json.name.is_some() && self.json.exports.is_some()
  }

  pub fn is_workspace(&self) -> bool {
    self.json.workspace.is_some()
  }

  pub fn has_unstable(&self, name: &str) -> bool {
    self.json.unstable.iter().any(|v| v == name)
  }

  /// Resolve the export values in a config file to their URLs.
  pub fn resolve_export_value_urls(
    &self,
  ) -> Result<Vec<Url>, ResolveExportValueUrlsError> {
    let exports_config = self
      .to_exports_config()
      .map_err(|error| ResolveExportValueUrlsError::ExportsConfig {
        specifier: self.specifier.clone(),
        error: Box::new(error),
      })?
      .into_map();
    let mut exports = Vec::with_capacity(exports_config.len());
    for (_, value) in exports_config {
      let entry_point = self.specifier.join(&value).map_err(|error| {
        ResolveExportValueUrlsError::JoinError {
          specifier: self.specifier.clone(),
          value: value.to_string(),
          error,
        }
      })?;
      exports.push(entry_point);
    }
    Ok(exports)
  }

  pub fn to_exports_config(
    &self,
  ) -> Result<ExportsConfig, ConfigFileExportsError> {
    fn has_extension(value: &str) -> bool {
      let search_text = &value[value.rfind('/').unwrap_or(0)..];
      search_text.contains('.')
    }

    fn validate_key(
      key_display: &dyn Fn() -> Cow<'static, str>,
      key: &str,
    ) -> Result<(), ConfigFileExportsError> {
      if key == "." {
        return Ok(());
      }
      if key.is_empty() {
        return Err(ConfigFileExportsError::KeyMustNotBeEmpty(key_display()));
      }
      if !key.starts_with("./") {
        let suggestion = if key.starts_with('/') {
          format!(".{}", key)
        } else {
          format!("./{}", key)
        };
        return Err(ConfigFileExportsError::KeyMustStartWithDotSlash {
          key: key_display(),
          suggestion,
        });
      }
      if key.ends_with('/') {
        let suggestion = key.trim_end_matches('/');
        return Err(ConfigFileExportsError::KeyMustNotEndWithSlash {
          key: key_display(),
          suggestion: suggestion.to_string(),
        });
      }
      // ban anything that is not [a-zA-Z0-9_-./]
      if key.chars().any(|c| {
        !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.' | '/')
      }) {
        return Err(ConfigFileExportsError::KeyInvalidCharacter(key_display()));
      }
      // ban parts consisting of only dots, and empty parts (e.g. `./foo//bar`)
      for part in key.split('/').skip(1) {
        if part.is_empty() || part.chars().all(|c| c == '.') {
          return Err(ConfigFileExportsError::KeyTooManySlashesOrDots(
            key_display(),
          ));
        }
      }
      Ok(())
    }

    fn validate_value(
      key_display: &dyn Fn() -> Cow<'static, str>,
      value: &str,
    ) -> Result<(), ConfigFileExportsError> {
      if value.is_empty() {
        return Err(ConfigFileExportsError::ValueMustNotBeEmpty(key_display()));
      }
      if !value.starts_with("./") {
        let suggestion = if value.starts_with('/') {
          format!(".{}", value)
        } else {
          format!("./{}", value)
        };
        return Err(ConfigFileExportsError::ValueCouldNotBeResolved {
          value: value.to_string(),
          key: key_display(),
          suggestion,
        });
      }
      if value.ends_with('/') {
        let suggestion = value.trim_end_matches('/');
        return Err(ConfigFileExportsError::ValueMustNotEndWithSlash {
          value: value.to_string(),
          key: key_display(),
          suggestion: suggestion.to_string(),
        });
      }
      if !has_extension(value) {
        return Err(ConfigFileExportsError::ValueMissingFileExtension {
          value: value.to_string(),
          key: key_display(),
        });
      }
      Ok(())
    }

    let map = match &self.json.exports {
      Some(Value::Object(map)) => {
        let mut result = IndexMap::with_capacity(map.len());
        for (k, v) in map {
          let key_display = || Cow::Owned(format!("'{}' export", k));
          validate_key(&key_display, k)?;
          match v {
            Value::String(value) => {
              validate_value(&key_display, value)?;
              result.insert(k.clone(), value.clone());
            }
            Value::Object(_) => {
              return Err(
                ConfigFileExportsError::InvalidValueConditionalExports {
                  key: key_display(),
                  value: v.clone(),
                },
              );
            }
            Value::Bool(_)
            | Value::Number(_)
            | Value::Array(_)
            | Value::Null => {
              return Err(ConfigFileExportsError::InvalidValue {
                key: key_display(),
                value: v.clone(),
              });
            }
          }
        }
        result
      }
      Some(Value::String(value)) => {
        validate_value(&|| "root export".into(), value)?;
        IndexMap::from([(".".to_string(), value.clone())])
      }
      Some(
        v @ Value::Bool(_)
        | v @ Value::Array(_)
        | v @ Value::Number(_)
        | v @ Value::Null,
      ) => {
        return Err(ConfigFileExportsError::ExportsKeyInvalidValue(v.clone()));
      }
      None => IndexMap::new(),
    };

    Ok(ExportsConfig {
      base: self.specifier.clone(),
      map,
    })
  }

  pub fn to_exclude_files_config(
    &self,
  ) -> Result<FilePatterns, ToInvalidConfigError> {
    let exclude = self.resolve_exclude_patterns()?;
    let raw_files_config = SerializedFilesConfig {
      exclude,
      ..Default::default()
    };
    raw_files_config
      .into_resolved(&self.specifier)
      .map_err(|error| ToInvalidConfigError::InvalidConfig {
        config: "exclude",
        source: error,
      })
  }

  fn resolve_exclude_patterns(
    &self,
  ) -> Result<Vec<String>, ToInvalidConfigError> {
    let mut exclude: Vec<String> =
      if let Some(exclude) = self.json.exclude.clone() {
        serde_json::from_value(exclude).map_err(|error| {
          ToInvalidConfigError::Parse {
            config: "exclude",
            source: error,
          }
        })?
      } else {
        Vec::new()
      };

    if self.json.vendor == Some(true) {
      exclude.push("vendor".to_string());
    }
    Ok(exclude)
  }

  pub fn to_bench_config(&self) -> Result<BenchConfig, ToInvalidConfigError> {
    match self.json.bench.clone() {
      Some(config) => {
        let mut exclude_patterns = self.resolve_exclude_patterns()?;
        let mut serialized: SerializedBenchConfig =
          serde_json::from_value(config).map_err(|error| {
            ToInvalidConfigError::Parse {
              config: "bench",
              source: error,
            }
          })?;
        // top level excludes at the start because they're lower priority
        exclude_patterns.extend(std::mem::take(&mut serialized.exclude));
        serialized.exclude = exclude_patterns;
        serialized.into_resolved(&self.specifier).map_err(|error| {
          ToInvalidConfigError::InvalidConfig {
            config: "bench",
            source: error,
          }
        })
      }
      None => Ok(BenchConfig {
        files: self.to_exclude_files_config()?,
      }),
    }
  }

  pub fn to_fmt_config(&self) -> Result<FmtConfig, ToInvalidConfigError> {
    match self.json.fmt.clone() {
      Some(config) => {
        let mut exclude_patterns = self.resolve_exclude_patterns()?;
        let mut serialized: SerializedFmtConfig =
          serde_json::from_value(config).map_err(|error| {
            ToInvalidConfigError::Parse {
              config: "fmt",
              source: error,
            }
          })?;
        // top level excludes at the start because they're lower priority
        exclude_patterns.extend(std::mem::take(&mut serialized.exclude));
        serialized.exclude = exclude_patterns;
        serialized.into_resolved(&self.specifier).map_err(|error| {
          ToInvalidConfigError::InvalidConfig {
            config: "fmt",
            source: error,
          }
        })
      }
      None => Ok(FmtConfig {
        options: Default::default(),
        files: self.to_exclude_files_config()?,
      }),
    }
  }

  pub fn to_lint_config(&self) -> Result<LintConfig, ToInvalidConfigError> {
    match self.json.lint.clone() {
      Some(config) => {
        let mut exclude_patterns = self.resolve_exclude_patterns()?;
        let mut serialized: SerializedLintConfig =
          serde_json::from_value(config).map_err(|error| {
            ToInvalidConfigError::Parse {
              config: "lint",
              source: error,
            }
          })?;
        // top level excludes at the start because they're lower priority
        exclude_patterns.extend(std::mem::take(&mut serialized.exclude));
        serialized.exclude = exclude_patterns;
        serialized.into_resolved(&self.specifier).map_err(|error| {
          ToInvalidConfigError::InvalidConfig {
            config: "lint",
            source: error,
          }
        })
      }
      None => Ok(LintConfig {
        options: Default::default(),
        files: self.to_exclude_files_config()?,
      }),
    }
  }

  pub fn to_test_config(&self) -> Result<TestConfig, ToInvalidConfigError> {
    match self.json.test.clone() {
      Some(config) => {
        let mut exclude_patterns = self.resolve_exclude_patterns()?;
        let mut serialized: SerializedTestConfig =
          serde_json::from_value(config).map_err(|error| {
            ToInvalidConfigError::Parse {
              config: "test",
              source: error,
            }
          })?;
        // top level excludes at the start because they're lower priority
        exclude_patterns.extend(std::mem::take(&mut serialized.exclude));
        serialized.exclude = exclude_patterns;
        serialized.into_resolved(&self.specifier).map_err(|error| {
          ToInvalidConfigError::InvalidConfig {
            config: "test",
            source: error,
          }
        })
      }
      None => Ok(TestConfig {
        files: self.to_exclude_files_config()?,
      }),
    }
  }

  pub(crate) fn to_publish_config(
    &self,
  ) -> Result<PublishConfig, ToInvalidConfigError> {
    match self.json.publish.clone() {
      Some(config) => {
        let mut exclude_patterns = self.resolve_exclude_patterns()?;
        let mut serialized: SerializedPublishConfig =
          serde_json::from_value(config).map_err(|error| {
            ToInvalidConfigError::Parse {
              config: "publish",
              source: error,
            }
          })?;
        // top level excludes at the start because they're lower priority
        exclude_patterns.extend(std::mem::take(&mut serialized.exclude));
        serialized.exclude = exclude_patterns;
        serialized.into_resolved(&self.specifier).map_err(|error| {
          ToInvalidConfigError::InvalidConfig {
            config: "public",
            source: error,
          }
        })
      }
      None => Ok(PublishConfig {
        files: self.to_exclude_files_config()?,
      }),
    }
  }

  pub fn to_link_config(
    &self,
  ) -> Result<Option<Vec<String>>, LinkConfigParseError> {
    match self
      .json
      .links
      .clone()
      .or(self.json.deprecated_patch.clone())
    {
      Some(config) => match config {
        Value::Null => Ok(None),
        config => {
          let members: Vec<String> =
            serde_json::from_value(config).map_err(LinkConfigParseError)?;
          Ok(Some(members))
        }
      },
      None => Ok(None),
    }
  }

  pub fn to_workspace_config(
    &self,
  ) -> Result<Option<WorkspaceConfig>, WorkspaceConfigParseError> {
    match self.json.workspace.clone() {
      Some(config) => match config {
        Value::Null => Ok(None),
        Value::Array(_) => {
          let members: Vec<String> = serde_json::from_value(config)
            .map_err(WorkspaceConfigParseError)?;
          Ok(Some(WorkspaceConfig { members }))
        }
        _ => {
          let config: WorkspaceConfig = serde_json::from_value(config)
            .map_err(WorkspaceConfigParseError)?;
          Ok(Some(config))
        }
      },
      None => Ok(None),
    }
  }

  pub fn to_license(&self) -> Option<String> {
    self.json.license.as_ref().and_then(|value| match value {
      Value::String(license) if !license.trim().is_empty() => {
        Some(license.trim().to_string())
      }
      _ => None,
    })
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
  ) -> Result<Option<IndexMap<String, TaskDefinition>>, ToInvalidConfigError>
  {
    if let Some(config) = self.json.tasks.clone() {
      let tasks_config: IndexMap<String, TaskDefinition> =
        TaskDefinition::deserialize_tasks(config).map_err(|error| {
          ToInvalidConfigError::Parse {
            config: "tasks",
            source: error,
          }
        })?;
      Ok(Some(tasks_config))
    } else {
      Ok(None)
    }
  }

  pub fn to_compiler_option_types(
    &self,
  ) -> Result<Option<(Url, Vec<String>)>, CompilerOptionTypesDeserializeError>
  {
    let Some(compiler_options_value) = self.json.compiler_options.as_ref()
    else {
      return Ok(None);
    };
    let Some(types) = compiler_options_value.get("types") else {
      return Ok(None);
    };
    let imports: Vec<String> =
      serde_json::from_value(types.clone()).map_err(|source| {
        CompilerOptionTypesDeserializeError {
          specifier: self.specifier.clone(),
          source,
        }
      })?;
    if !imports.is_empty() {
      let referrer = self.specifier.clone();
      Ok(Some((referrer, imports)))
    } else {
      Ok(None)
    }
  }

  /// Based on the compiler options in the configuration file, return the
  /// JSX import source configuration.
  pub fn to_raw_jsx_compiler_options(&self) -> RawJsxCompilerOptions {
    self
      .json
      .compiler_options
      .as_ref()
      .and_then(|compiler_options_value| {
        serde_json::from_value::<RawJsxCompilerOptions>(
          compiler_options_value.clone(),
        )
        .ok()
      })
      .unwrap_or_default()
  }

  pub fn resolve_tasks_config(
    &self,
  ) -> Result<IndexMap<String, TaskDefinition>, ResolveTaskConfigError> {
    let maybe_tasks_config = self.to_tasks_config()?;
    let tasks_config = maybe_tasks_config.unwrap_or_default();
    for key in tasks_config.keys() {
      if key.is_empty() {
        return Err(ResolveTaskConfigError::TaskNameEmpty);
      } else if !key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':'))
      {
        return Err(ResolveTaskConfigError::TaskNameInvalidCharacter(
          key.to_string(),
        ));
      } else if !key.chars().next().unwrap().is_ascii_alphabetic() {
        return Err(ResolveTaskConfigError::TaskNameInvalidStartingCharacter(
          key.to_string(),
        ));
      }
    }
    Ok(tasks_config)
  }

  pub fn to_lock_config(
    &self,
  ) -> Result<Option<LockConfig>, ToLockConfigError> {
    if let Some(config) = self.json.lock.clone() {
      let mut lock_config: LockConfig = serde_json::from_value(config)
        .map_err(|error| ToInvalidConfigError::Parse {
          config: "lock",
          source: error,
        })?;
      if let LockConfig::PathBuf(path)
      | LockConfig::Object {
        path: Some(path), ..
      } = &mut lock_config
      {
        *path = url_to_file_path(&self.specifier)?
          .parent()
          .unwrap()
          .join(&path);
      }
      Ok(Some(lock_config))
    } else {
      Ok(None)
    }
  }

  pub fn resolve_lockfile_path(
    &self,
  ) -> Result<Option<PathBuf>, ToLockConfigError> {
    match self.to_lock_config()? {
      Some(LockConfig::Bool(lock)) if !lock => Ok(None),
      Some(LockConfig::PathBuf(lock)) => Ok(Some(lock)),
      Some(LockConfig::Object { path, .. }) if path.is_some() => Ok(path),
      _ => {
        let mut path = url_to_file_path(&self.specifier)?;
        path.set_file_name("deno.lock");
        Ok(Some(path))
      }
    }
  }
}

/// Represents the "default" type library that should be used when type
/// checking the code in the module graph.  Note that a user provided config
/// of `"lib"` would override this value.
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum TsTypeLib {
  DenoWindow,
  DenoWorker,
}

impl Default for TsTypeLib {
  fn default() -> Self {
    Self::DenoWindow
  }
}

impl Serialize for TsTypeLib {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let value = match self {
      Self::DenoWindow => {
        vec!["deno.window".to_string(), "deno.unstable".to_string()]
      }
      Self::DenoWorker => {
        vec!["deno.worker".to_string(), "deno.unstable".to_string()]
      }
    };
    Serialize::serialize(&value, serializer)
  }
}

/// An enum that represents the base tsc configuration to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompilerOptionsType {
  /// Return a configuration for bundling, using swc to emit the bundle. This is
  /// independent of type checking.
  Bundle,
  /// Return a configuration to use tsc to type check. This
  /// is independent of either bundling or emitting via swc.
  Check { lib: TsTypeLib },
  /// Return a configuration to use swc to emit single module files.
  Emit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerOptionsWithIgnoredOptions {
  pub compiler_options: CompilerOptions,
  pub ignored_options: Vec<IgnoredCompilerOptions>,
}

/// For a given configuration type get the starting point CompilerOptions
/// used that can then be merged with user specified options.
pub fn get_base_compiler_options_for_emit(
  config_type: CompilerOptionsType,
) -> CompilerOptions {
  match config_type {
    CompilerOptionsType::Bundle => CompilerOptions::new(json!({
      "allowImportingTsExtensions": true,
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "experimentalDecorators": true,
      "importsNotUsedAsValues": "remove",
      "inlineSourceMap": false,
      "inlineSources": false,
      "sourceMap": false,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
      "module": "NodeNext",
      "moduleResolution": "NodeNext",
    })),
    CompilerOptionsType::Check { lib } => CompilerOptions::new(json!({
      "allowJs": true,
      "allowImportingTsExtensions": true,
      "allowSyntheticDefaultImports": true,
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "experimentalDecorators": false,
      "incremental": true,
      "jsx": "react",
      "importsNotUsedAsValues": "remove",
      "inlineSourceMap": true,
      "inlineSources": true,
      "isolatedModules": true,
      "lib": lib,
      "module": "NodeNext",
      "moduleResolution": "NodeNext",
      "moduleDetection": "force",
      "noEmit": true,
      "noImplicitOverride": true,
      "resolveJsonModule": true,
      "sourceMap": false,
      "strict": true,
      "target": "esnext",
      "tsBuildInfoFile": "internal:///.tsbuildinfo",
      "useDefineForClassFields": true,
    })),
    CompilerOptionsType::Emit => CompilerOptions::new(json!({
      "allowImportingTsExtensions": true,
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "experimentalDecorators": false,
      "importsNotUsedAsValues": "remove",
      "inlineSourceMap": true,
      "inlineSources": true,
      "sourceMap": false,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
      "module": "NodeNext",
      "moduleResolution": "NodeNext",
      "resolveJsonModule": true,
    })),
  }
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use deno_path_util::url_to_file_path;
  use pretty_assertions::assert_eq;
  use sys_traits::impls::RealSys;

  use super::*;
  use crate::glob::PathOrPattern;

  #[macro_export]
  macro_rules! assert_contains {
    ($string:expr, $($test:expr),+ $(,)?) => {
      let string = &$string; // This might be a function call or something
      if !($(string.contains($test))||+) {
        panic!("{:?} does not contain any of {:?}", string, [$($test),+]);
      }
    }
  }

  struct UnreachableSys;

  impl sys_traits::BaseFsRead for UnreachableSys {
    fn base_fs_read(
      &self,
      _path: &Path,
    ) -> std::io::Result<Cow<'static, [u8]>> {
      unreachable!()
    }
  }

  fn testdata_path() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"))).join("testdata")
  }

  fn unpack_object<T>(
    result: Result<T, ToInvalidConfigError>,
    name: &str,
  ) -> T {
    result
      .unwrap_or_else(|err| panic!("error parsing {name} object but got {err}"))
  }

  #[test]
  fn read_config_file_absolute() {
    let path = testdata_path().join("module_graph/tsconfig.json");
    let config_file = ConfigFile::read(&RealSys, path.as_path()).unwrap();
    assert!(config_file.json.compiler_options.is_some());
  }

  #[test]
  fn include_config_path_on_error() {
    let path = testdata_path().join("404.json");
    let error = ConfigFile::read(&RealSys, path.as_path()).err().unwrap();
    assert!(error.to_string().contains("404.json"));
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
        "include": ["src/"],
        "exclude": ["src/testdata/"],
        "rules": {
          "tags": ["recommended"],
          "include": ["ban-untagged-todo"]
        }
      },
      "fmt": {
        "include": ["src/"],
        "exclude": ["src/testdata/"],
        "useTabs": true,
        "lineWidth": 80,
        "indentWidth": 4,
        "singleQuote": true,
        "proseWrap": "preserve",
        "quoteProps": "asNeeded",
        "newLineKind": "crlf",
        "useBraces": "whenNotSingleLine",
        "bracePosition": "sameLine",
        "singleBodyPosition": "nextLine",
        "nextControlFlowPosition": "sameLine",
        "trailingCommas": "never",
        "operatorPosition": "maintain",
        "jsx.bracketPosition": "maintain",
        "jsx.forceNewLinesSurroundingContent": true,
        "jsx.multiLineParens": "never",
        "typeLiteral.separatorKind": "semiColon",
        "spaceAround": true,
        "spaceSurroundingProperties": true
      },
      "tasks": {
        "build": "deno run --allow-read --allow-write build.ts",
        "server": "deno run --allow-net --allow-read server.ts",
        "client": {
          "description": "Build client project",
          "command": "deno run -A client.js",
          "dependencies": ["build"]
        }
      },
      "unstable": ["kv", "ffi"]
    }"#;
    let config_dir = Url::parse("file:///deno/").unwrap();
    let config_specifier = config_dir.join("tsconfig.json").unwrap();
    let config_file =
      ConfigFile::new(config_text, config_specifier.clone()).unwrap();
    let ParsedCompilerOptions {
      options,
      maybe_ignored,
    } = config_file
      .to_compiler_options()
      .unwrap()
      .unwrap_or_default();
    assert!(options.contains_key("strict"));
    assert_eq!(options.len(), 1);
    assert_eq!(
      maybe_ignored,
      Some(IgnoredCompilerOptions {
        items: vec!["build".to_string()],
        maybe_specifier: Some(config_specifier),
      }),
    );

    let config_dir_path = url_to_file_path(&config_dir).unwrap();
    assert_eq!(
      unpack_object(config_file.to_lint_config(), "lint"),
      LintConfig {
        files: FilePatterns {
          base: config_dir_path.clone(),
          include: Some(PathOrPatternSet::new(vec![PathOrPattern::Path(
            PathBuf::from("/deno/src/")
          )])),
          exclude: PathOrPatternSet::new(vec![PathOrPattern::Path(
            PathBuf::from("/deno/src/testdata/")
          )]),
        },
        options: LintOptionsConfig {
          rules: LintRulesConfig {
            include: Some(vec!["ban-untagged-todo".to_string()]),
            exclude: None,
            tags: Some(vec!["recommended".to_string()]),
          },
          plugins: vec![],
        }
      }
    );
    assert_eq!(
      unpack_object(config_file.to_fmt_config(), "fmt"),
      FmtConfig {
        files: FilePatterns {
          base: config_dir_path.clone(),
          include: Some(PathOrPatternSet::new(vec![PathOrPattern::Path(
            PathBuf::from("/deno/src/")
          )])),
          exclude: PathOrPatternSet::new(vec![PathOrPattern::Path(
            PathBuf::from("/deno/src/testdata/")
          )]),
        },
        options: FmtOptionsConfig {
          use_tabs: Some(true),
          line_width: Some(80),
          indent_width: Some(4),
          single_quote: Some(true),
          semi_colons: None,
          prose_wrap: Some(ProseWrap::Preserve),
          quote_props: Some(QuoteProps::AsNeeded),
          new_line_kind: Some(NewLineKind::CarriageReturnLineFeed),
          use_braces: Some(UseBraces::WhenNotSingleLine),
          brace_position: Some(BracePosition::SameLine),
          single_body_position: Some(SingleBodyPosition::NextLine),
          next_control_flow_position: Some(NextControlFlowPosition::SameLine),
          trailing_commas: Some(TrailingCommas::Never),
          operator_position: Some(OperatorPosition::Maintain),
          jsx_bracket_position: Some(BracketPosition::Maintain),
          jsx_force_new_lines_surrounding_content: Some(true),
          jsx_multi_line_parens: Some(MultiLineParens::Never),
          type_literal_separator_kind: Some(SeparatorKind::SemiColon),
          space_around: Some(true),
          space_surrounding_properties: Some(true),
        },
      }
    );

    let tasks_config = config_file.to_tasks_config().unwrap().unwrap();
    assert_eq!(
      tasks_config["build"],
      "deno run --allow-read --allow-write build.ts".into(),
    );
    assert_eq!(
      tasks_config["server"],
      "deno run --allow-net --allow-read server.ts".into(),
    );
    assert_eq!(
      tasks_config["client"],
      TaskDefinition {
        description: Some("Build client project".to_string()),
        command: Some("deno run -A client.js".to_string()),
        dependencies: vec!["build".to_string()]
      }
    );

    assert_eq!(
      config_file.json.unstable,
      vec!["kv".to_string(), "ffi".to_string()],
    )
  }

  #[test]
  fn test_parse_config_exclude_lower_priority_path() {
    let config_text = r#"{
      "fmt": {
        "exclude": ["!dist/data", "dist/"]
      }
    }"#;
    let config_specifier = Url::parse("file:///deno/tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();

    let err = config_file.to_fmt_config().err().unwrap();
    assert_eq!(err.to_string(), "Invalid fmt config");
    assert_eq!(
      std::error::Error::source(&err).unwrap().to_string(),
      r#"Invalid exclude: The negation of '!dist/data' is never reached due to the higher priority 'dist/' exclude. Move '!dist/data' after 'dist/'."#
    );
  }

  #[test]
  fn test_parse_config_exclude_lower_priority_glob() {
    let config_text = r#"{
      "lint": {
        "exclude": ["!dist/data/**/*.ts", "dist/"]
      }
    }"#;
    let config_specifier = Url::parse("file:///deno/tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();

    let err = config_file.to_lint_config().err().unwrap();
    assert_eq!(err.to_string(), "Invalid lint config");
    assert_eq!(
      std::error::Error::source(&err).unwrap().to_string(),
      r#"Invalid exclude: The negation of '!dist/data/**/*.ts' is never reached due to the higher priority 'dist/' exclude. Move '!dist/data/**/*.ts' after 'dist/'."#
    );
  }

  #[test]
  fn test_parse_config_with_deprecated_fmt_options() {
    let config_text_both = r#"{
      "fmt": {
        "options": {
          "semiColons": true
        },
        "semiColons": false
      }
    }"#;
    let config_text_deprecated = r#"{
      "fmt": {
        "options": {
          "semiColons": true
        }
      }
    }"#;
    let config_specifier = Url::parse("file:///deno/tsconfig.json").unwrap();
    let config_file_both =
      ConfigFile::new(config_text_both, config_specifier.clone()).unwrap();
    let config_file_deprecated =
      ConfigFile::new(config_text_deprecated, config_specifier).unwrap();

    fn unpack_options(config_file: ConfigFile) -> FmtOptionsConfig {
      unpack_object(config_file.to_fmt_config(), "fmt").options
    }

    let fmt_options_both = unpack_options(config_file_both);
    assert_eq!(fmt_options_both.semi_colons, Some(false));

    let fmt_options_deprecated = unpack_options(config_file_deprecated);
    assert_eq!(fmt_options_deprecated.semi_colons, Some(true));
  }

  #[test]
  fn test_parse_config_with_empty_file() {
    let config_text = "";
    let config_specifier = Url::parse("file:///deno/tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
    config_file.to_compiler_options().unwrap(); // no panic
  }

  #[test]
  fn test_parse_config_with_commented_file() {
    let config_text = r#"//{"foo":"bar"}"#;
    let config_specifier = Url::parse("file:///deno/tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
    config_file.to_compiler_options().unwrap(); // no panic
  }

  #[test]
  fn test_parse_config_with_global_files() {
    let config_text = r#"{
      "exclude": ["foo/"],
      "test": {
        "exclude": ["npm/"],
      },
      "bench": {}
    }"#;
    let config_specifier = Url::parse("file:///deno/tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();

    config_file.to_compiler_options().unwrap(); // no panic

    let test_config = config_file.to_test_config().unwrap();
    assert_eq!(test_config.files.include, None);
    assert_eq!(
      test_config.files.exclude,
      PathOrPatternSet::from_absolute_paths(&[
        "/deno/foo/".to_string(),
        "/deno/npm/".to_string(),
      ])
      .unwrap()
    );

    let bench_config = config_file.to_bench_config().unwrap();
    assert_eq!(
      bench_config.files.exclude,
      PathOrPatternSet::from_absolute_paths(&["/deno/foo/".to_string()])
        .unwrap()
    );
  }

  #[test]
  fn test_parse_config_publish() {
    let config_text = r#"{
      "exclude": ["foo/"],
      "publish": {
        "exclude": ["npm/"],
      }
    }"#;
    let config_specifier = Url::parse("file:///deno/tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();

    config_file.to_compiler_options().unwrap(); // no panic

    let publish_config = config_file.to_publish_config().unwrap();
    assert_eq!(publish_config.files.include, None);
    assert_eq!(
      publish_config.files.exclude,
      PathOrPatternSet::from_absolute_paths(&[
        "/deno/foo/".to_string(),
        "/deno/npm/".to_string(),
      ])
      .unwrap()
    );
  }

  #[test]
  fn test_parse_config_with_global_files_only() {
    let config_text = r#"{
      "exclude": ["npm/"]
    }"#;
    let config_specifier = Url::parse("file:///deno/tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();

    config_file.to_compiler_options().unwrap(); // no panic

    let files_config = config_file.to_exclude_files_config().unwrap();
    assert_eq!(files_config.include, None);
    assert_eq!(
      files_config.exclude,
      PathOrPatternSet::from_absolute_paths(&["/deno/npm/".to_string()])
        .unwrap()
    );

    let lint_config = config_file.to_lint_config().unwrap();
    assert_eq!(lint_config.files.include, None);
    assert_eq!(
      lint_config.files.exclude,
      PathOrPatternSet::from_absolute_paths(&["/deno/npm/".to_string()])
        .unwrap()
    );

    let fmt_config = config_file.to_fmt_config().unwrap();
    assert_eq!(fmt_config.files.include, None);
    assert_eq!(
      fmt_config.files.exclude,
      PathOrPatternSet::from_absolute_paths(&["/deno/npm/".to_string()])
        .unwrap()
    );
  }

  #[test]
  fn test_parse_config_with_invalid_file() {
    let config_text = "{foo:bar}";
    let config_specifier = Url::parse("file:///deno/tsconfig.json").unwrap();
    // Emit error: Unable to parse config file JSON "<config_path>" because of Unexpected token on line 1 column 6.
    assert!(ConfigFile::new(config_text, config_specifier,).is_err());
  }

  #[test]
  fn test_parse_config_with_not_object_file() {
    let config_text = "[]";
    let config_specifier = Url::parse("file:///deno/tsconfig.json").unwrap();
    // Emit error: config file JSON "<config_path>" should be an object
    assert!(ConfigFile::new(config_text, config_specifier,).is_err());
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

  #[track_caller]
  fn run_task_error_test(config_text: &str, expected_error: &str) {
    let config_dir = Url::parse("file:///deno/").unwrap();
    let config_specifier = config_dir.join("tsconfig.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
    assert_eq!(
      config_file.resolve_tasks_config().unwrap_err().to_string(),
      expected_error,
    );
  }

  #[test]
  fn files_pattern_matches_remote() {
    assert!(FilePatterns::new_with_base(PathBuf::from("/"))
      .matches_specifier(&Url::parse("https://example.com/mod.ts").unwrap()));
  }

  #[test]
  fn resolve_lockfile_path_from_unix_path() {
    let config_file =
      ConfigFile::new("{}", Url::parse("file:///root/deno.json").unwrap())
        .unwrap();
    let lockfile_path = config_file.resolve_lockfile_path().unwrap();
    let lockfile_path = lockfile_path.unwrap();
    assert_eq!(lockfile_path, PathBuf::from("/root/deno.lock"));
  }

  #[test]
  fn exports() {
    fn get_exports(config_text: &str) -> ExportsConfig {
      let config_dir = Url::parse("file:///deno/").unwrap();
      let config_specifier = config_dir.join("tsconfig.json").unwrap();
      let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
      config_file.to_exports_config().unwrap()
    }

    // no exports
    assert_eq!(
      get_exports("{}").into_map(),
      IndexMap::<String, String>::new()
    );
    // string export
    assert_eq!(
      get_exports(r#"{ "exports": "./mod.ts" }"#).into_map(),
      IndexMap::from([(".".to_string(), "./mod.ts".to_string())])
    );
    // map export
    assert_eq!(
      get_exports(r#"{ "exports": { "./export": "./mod.ts" } }"#).into_map(),
      IndexMap::from([("./export".to_string(), "./mod.ts".to_string())])
    );
    // resolve an export
    let exports = get_exports(r#"{ "exports": { "./export": "./mod.ts" } }"#);
    assert_eq!(
      exports
        .get_resolved("./export")
        .unwrap()
        .unwrap()
        .to_string(),
      "file:///deno/mod.ts"
    );
    assert!(exports.get_resolved("./non-existent").unwrap().is_none());
  }

  #[test]
  fn exports_errors() {
    #[track_caller]
    fn run_test(config_text: &str, expected_error: &str) {
      let config_dir = Url::parse("file:///deno/").unwrap();
      let config_specifier = config_dir.join("tsconfig.json").unwrap();
      let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
      assert_eq!(
        config_file.to_exports_config().unwrap_err().to_string(),
        expected_error,
      );
    }

    // empty key
    run_test(
      r#"{ "exports": { "": "./mod.ts" } }"#,
      "The '' export must not be empty. Use '.' if you meant the root export.",
    );
    // no ./ at start of key
    run_test(
      r#"{ "exports": { "mod": "./mod.ts" } }"#,
      "The 'mod' export must start with './'. Did you mean './mod'?",
    );
    // trailing slash in key
    run_test(
      r#"{ "exports": { "./mod/": "./mod.ts" } }"#,
      "The './mod/' export must not end with '/'. Did you mean './mod'?",
    );
    // multiple trailing slash in key
    run_test(
      r#"{ "exports": { "./mod//": "./mod.ts" } }"#,
      "The './mod//' export must not end with '/'. Did you mean './mod'?",
    );
    // unsupported characters in key
    run_test(
      r#"{ "exports": { "./mod*": "./mod.ts" } }"#,
      "The './mod*' export must only contain alphanumeric characters, underscores (_), dashes (-), dots (.), and slashes (/).",
    );
    // double slash in key
    run_test(
      r#"{ "exports": { "./mod//bar": "./mod.ts" } }"#,
      "The './mod//bar' export must not contain double slashes (//), or parts consisting entirely of dots (.).",
    );
    // . part in key
    run_test(
      r#"{ "exports": { "././mod": "./mod.ts" } }"#,
      "The '././mod' export must not contain double slashes (//), or parts consisting entirely of dots (.).",
    );
    // .. part in key
    run_test(
      r#"{ "exports": { "./../mod": "./mod.ts" } }"#,
      "The './../mod' export must not contain double slashes (//), or parts consisting entirely of dots (.).",
    );
    // ...... part in key
    run_test(
      r#"{ "exports": { "./....../mod": "./mod.ts" } }"#,
      "The './....../mod' export must not contain double slashes (//), or parts consisting entirely of dots (.).",
    );

    // empty value
    run_test(
      r#"{ "exports": { "./mod": "" } }"#,
      "The path for the './mod' export must not be empty.",
    );
    // value without ./ at start
    run_test(
      r#"{ "exports": { "./mod": "mod.ts" } }"#,
      "The path 'mod.ts' at the './mod' export could not be resolved as a relative path from the config file. Did you mean './mod.ts'?",
    );
    // value with a trailing slash
    run_test(
      r#"{ "exports": { "./mod": "./folder/" } }"#,
      "The path './folder/' at the './mod' export must not end with '/'. Did you mean './folder'?",
    );
    // value without an extension
    run_test(
      r#"{ "exports": { "./mod": "./folder" } }"#,
      "The path './folder' at the './mod' export is missing a file extension. Add a file extension such as '.js' or '.ts'.",
    );
    // boolean key value
    run_test(
      r#"{ "exports": { "./mod": true } }"#,
      "The path of the './mod' export must be a string, found invalid value 'true'.",
    );
    // object key value
    run_test(
      r#"{ "exports": { "./mod": {} } }"#,
      "The path of the './mod' export must be a string, found invalid value '{}'. Exports in deno.json do not support conditional exports.",
    );
    // non-map or string value
    run_test(
      r#"{ "exports": [] }"#,
      "The 'exports' key must be a string or object, found invalid value '[]'.",
    );
    // null
    run_test(
      r#"{ "exports": { "./mod": null }  }"#,
      "The path of the './mod' export must be a string, found invalid value 'null'.",
    );
  }

  #[test]
  fn resolve_export_value_urls() {
    fn get_exports(config_text: &str) -> Vec<String> {
      let config_dir = Url::parse("file:///deno/").unwrap();
      let config_specifier = config_dir.join("tsconfig.json").unwrap();
      let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
      config_file
        .resolve_export_value_urls()
        .unwrap()
        .into_iter()
        .map(|u| u.to_string())
        .collect()
    }

    // no exports
    assert_eq!(get_exports("{}"), Vec::<String>::new());
    // string export
    assert_eq!(
      get_exports(r#"{ "exports": "./mod.ts" }"#),
      vec!["file:///deno/mod.ts".to_string()]
    );
    // map export
    assert_eq!(
      get_exports(r#"{ "exports": { "./export": "./mod.ts" } }"#),
      vec!["file:///deno/mod.ts".to_string()]
    );
    // multiple
    assert_eq!(
      get_exports(
        r#"{ "exports": { "./export": "./mod.ts", "./other": "./other.ts" } }"#
      ),
      vec![
        "file:///deno/mod.ts".to_string(),
        "file:///deno/other.ts".to_string(),
      ]
    );
  }

  #[test]
  fn test_is_package() {
    fn get_for_config(config_text: &str) -> bool {
      let config_specifier = root_url().join("tsconfig.json").unwrap();
      let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
      config_file.is_package()
    }

    assert!(!get_for_config("{}"));
    assert!(!get_for_config(
      r#"{
        "name": "test"
      }"#
    ));
    assert!(!get_for_config(
      r#"{
        "name": "test",
        "version": "1.0.0"
      }"#
    ));
    assert!(get_for_config(
      r#"{
        "name": "test",
        "exports": "./mod.ts"
      }"#
    ));
    assert!(!get_for_config(
      r#"{
        "version": "1.0.0",
        "exports": "./mod.ts"
      }"#
    ));
    assert!(get_for_config(
      r#"{
        "name": "test",
        "version": "1.0.0",
        "exports": "./mod.ts"
      }"#
    ));
  }

  #[test]
  fn test_to_import_map_from_imports() {
    let config_text = r#"{
      "imports": {
        "@std/test": "jsr:@std/test@0.2.0"
      }
    }"#;
    let config_specifier = root_url().join("deno.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
    let result = config_file.to_import_map_from_imports().unwrap();

    assert_eq!(
      json!(result.import_map.imports()),
      // imports should be expanded
      json!({
        "@std/test/": "jsr:/@std/test@0.2.0/",
        "@std/test": "jsr:@std/test@0.2.0",
      })
    );
  }

  #[test]
  fn test_to_import_map_imports_entry() {
    let config_text = r#"{
      "imports": { "@std/test": "jsr:@std/test@0.2.0" },
      // will be ignored because imports and scopes takes precedence
      "importMap": "import_map.json",
    }"#;
    let config_specifier = root_url().join("deno.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
    let result = config_file.to_import_map(&UnreachableSys).unwrap().unwrap();

    assert_eq!(
      result.import_map.base_url(),
      &root_url().join("deno.json").unwrap()
    );
    assert_eq!(
      json!(result.import_map.imports()),
      // imports should be expanded
      json!({
        "@std/test/": "jsr:/@std/test@0.2.0/",
        "@std/test": "jsr:@std/test@0.2.0",
      })
    );
  }

  #[test]
  fn test_to_import_map_scopes_entry() {
    let config_text = r#"{
      "scopes": { "https://deno.land/x/test/mod.ts": { "@std/test": "jsr:@std/test@0.2.0" } },
      // will be ignored because imports and scopes takes precedence
      "importMap": "import_map.json",
    }"#;
    let config_specifier = root_url().join("deno.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
    let result = config_file.to_import_map(&UnreachableSys).unwrap().unwrap();

    assert_eq!(
      result.import_map.base_url(),
      &root_url().join("deno.json").unwrap()
    );
    assert_eq!(
      json!(result.import_map),
      // imports should be expanded
      json!({
        "imports": {},
        "scopes": {
          "https://deno.land/x/test/mod.ts": {
            "@std/test/": "jsr:/@std/test@0.2.0/",
            "@std/test": "jsr:@std/test@0.2.0",
          }
        }
      })
    );
  }

  #[test]
  fn test_to_import_map_import_map_entry() {
    struct MockFs;

    impl sys_traits::BaseFsRead for MockFs {
      fn base_fs_read(
        &self,
        path: &Path,
      ) -> std::io::Result<Cow<'static, [u8]>> {
        assert_eq!(
          path,
          root_url().to_file_path().unwrap().join("import_map.json")
        );
        Ok(Cow::Borrowed(
          r#"{ "imports": { "@std/test": "jsr:@std/test@0.2.0" } }"#.as_bytes(),
        ))
      }
    }

    let config_text = r#"{
      "importMap": "import_map.json",
    }"#;
    let config_specifier = root_url().join("deno.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
    let result = config_file.to_import_map(&MockFs).unwrap().unwrap();

    assert_eq!(
      result.import_map.base_url(),
      &root_url().join("import_map.json").unwrap()
    );
    assert_eq!(
      json!(result.import_map.imports()),
      // imports should NOT be expanded
      json!({
        "@std/test": "jsr:@std/test@0.2.0",
      })
    );
  }

  #[test]
  fn test_to_import_map_import_map_remote() {
    let config_text = r#"{
      "importMap": "https://deno.land/import_map.json",
    }"#;
    let config_specifier = root_url().join("deno.json").unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
    let err = config_file.to_import_map(&UnreachableSys).unwrap_err();
    assert_eq!(
      err.to_string(),
      concat!(
        "Only file: specifiers are supported for security reasons in ",
        "import maps stored in a deno.json. To use a remote import map, ",
        "use the --import-map flag and \"deno.importMap\" in the ",
        "language server config"
      )
    );
  }

  fn root_url() -> Url {
    if cfg!(windows) {
      Url::parse("file://C:/deno/").unwrap()
    } else {
      Url::parse("file:///deno/").unwrap()
    }
  }

  #[test]
  fn task_comments() {
    let config_text = r#"{
      "tasks": {
        // dev task
        "dev": "deno run -A --watch mod.ts",
        // run task
        // with multiple line comments
        "run": "deno run -A mod.ts", // comments not supported here
        /*
         * test task
         *
         * with multi-line comments
         */
        "test": "deno test",
        /* we should */ /* ignore these */ "fmt": "deno fmt",
        "lint": "deno lint"
        // trailing comments
      },
    }"#;

    let config =
      ConfigFile::new(config_text, root_url().join("deno.jsonc").unwrap())
        .unwrap();
    assert_eq!(
      config.resolve_tasks_config().unwrap(),
      IndexMap::from([
        ("dev".into(), "deno run -A --watch mod.ts".into(),),
        ("run".into(), "deno run -A mod.ts".into(),),
        ("test".into(), "deno test".into(),),
        ("fmt".into(), "deno fmt".into(),),
        ("lint".into(), "deno lint".into(),)
      ])
    );
  }

  #[test]
  fn resolve_import_map_url_parent() {
    let config_text = r#"{ "importMap": "../import_map.json" }"#;
    let file_path = root_url()
      .join("sub/deno.json")
      .unwrap()
      .to_file_path()
      .unwrap();
    let config_specifier = Url::from_file_path(&file_path).unwrap();
    let config_file = ConfigFile::new(config_text, config_specifier).unwrap();
    assert_eq!(
      config_file.to_import_map_path().unwrap().unwrap(),
      file_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("import_map.json"),
    );
  }

  #[test]
  fn lock_object() {
    fn root_joined(path: &str) -> PathBuf {
      root_url().join(path).unwrap().to_file_path().unwrap()
    }
    let cases = [
      (
        r#"{ "lock": { "path": "mydeno.lock", "frozen": true } }"#,
        (true, root_joined("mydeno.lock")),
      ),
      (
        r#"{ "lock": { "frozen": false } }"#,
        (false, root_joined("deno.lock")),
      ),
      (
        r#"{ "lock": { "path": "mydeno.lock" } }"#,
        (false, root_joined("mydeno.lock")),
      ),
      (r#"{ "lock": {} }"#, (false, root_joined("deno.lock"))),
    ];
    for (config_text, (frozen, resolved_path)) in cases {
      let config_file =
        ConfigFile::new(config_text, root_url().join("deno.json").unwrap())
          .unwrap();
      let lock_config = config_file.to_lock_config().unwrap().unwrap();
      assert_eq!(
        config_file.resolve_lockfile_path().unwrap().unwrap(),
        resolved_path,
      );
      assert_eq!(lock_config.frozen(), frozen);
    }
  }

  #[test]
  fn node_modules_dir_mode() {
    let cases = [
      (json!("auto"), Ok(NodeModulesDirMode::Auto)),
      (json!("manual"), Ok(NodeModulesDirMode::Manual)),
      (json!("none"), Ok(NodeModulesDirMode::None)),
      (json!(true), Ok(NodeModulesDirMode::Auto)),
      (json!(false), Ok(NodeModulesDirMode::None)),
      (json!("other"), Err(r#"invalid value: string "other", expected "auto", "manual", or "none""#.into()))
    ];

    for (input, expected) in cases {
      assert_eq!(
        NodeModulesDirMode::deserialize(input).map_err(|e| e.to_string()),
        expected
      );
    }
  }
}
