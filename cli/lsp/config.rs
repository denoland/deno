// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::ops::Deref;
use std::ops::DerefMut;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_config::deno_json::DenoJsonCache;
use deno_config::deno_json::FmtConfig;
use deno_config::deno_json::FmtOptionsConfig;
use deno_config::deno_json::NodeModulesDirMode;
use deno_config::deno_json::TestConfig;
use deno_config::deno_json::TsConfig;
use deno_config::deno_json::TsConfigWithIgnoredOptions;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPatternSet;
use deno_config::workspace::JsxImportSourceConfig;
use deno_config::workspace::VendorEnablement;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceCache;
use deno_config::workspace::WorkspaceDirLintConfig;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceDirectoryEmptyOptions;
use deno_config::workspace::WorkspaceDiscoverOptions;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde::de::DeserializeOwned;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_lib::args::has_flag_env_var;
use deno_lib::util::hash::FastInsecureHasher;
use deno_lint::linter::LintConfig as DenoLintConfig;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_package_json::PackageJsonCache;
use deno_path_util::url_to_file_path;
use deno_resolver::npmrc::discover_npmrc_from_workspace;
use deno_resolver::workspace::CreateResolverOptions;
use deno_resolver::workspace::FsCacheOptions;
use deno_resolver::workspace::PackageJsonDepResolution;
use deno_resolver::workspace::SloppyImportsOptions;
use deno_resolver::workspace::SpecifiedImportMap;
use deno_resolver::workspace::WorkspaceResolver;
use deno_runtime::deno_node::PackageJson;
use indexmap::IndexSet;
use lsp_types::ClientCapabilities;
use lsp_types::Uri;
use tower_lsp::lsp_types as lsp;

use super::logging::lsp_log;
use super::lsp_custom;
use super::urls::uri_to_url;
use super::urls::url_to_uri;
use crate::args::CliLockfile;
use crate::args::CliLockfileReadFromPathOptions;
use crate::args::ConfigFile;
use crate::args::LintFlags;
use crate::args::LintOptions;
use crate::cache::DenoDir;
use crate::file_fetcher::CliFileFetcher;
use crate::lsp::logging::lsp_warn;
use crate::sys::CliSys;
use crate::tools::lint::CliLinter;
use crate::tools::lint::CliLinterOptions;
use crate::tools::lint::LintRuleProvider;
use crate::util::fs::canonicalize_path_maybe_not_exists;

pub const SETTINGS_SECTION: &str = "deno";

fn is_true() -> bool {
  true
}

/// Wrapper that defaults if it fails to deserialize. Good for individual
/// settings.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct SafeValue<T> {
  inner: T,
}

impl<'de, T: Default + for<'de2> Deserialize<'de2>> Deserialize<'de>
  for SafeValue<T>
{
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    Ok(Self {
      inner: Deserialize::deserialize(deserializer).unwrap_or_default(),
    })
  }
}

impl<T: Serialize> Serialize for SafeValue<T> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    self.inner.serialize(serializer)
  }
}

impl<T> Deref for SafeValue<T> {
  type Target = T;
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<T> DerefMut for SafeValue<T> {
  fn deref_mut(&mut self) -> &mut T {
    &mut self.inner
  }
}

impl<T> SafeValue<T> {
  pub fn as_deref(&self) -> &T {
    &self.inner
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensSettings {
  /// Flag for providing implementation code lenses.
  #[serde(default)]
  pub implementations: bool,
  /// Flag for providing reference code lenses.
  #[serde(default)]
  pub references: bool,
  /// Flag for providing reference code lens on all functions.  For this to have
  /// an impact, the `references` flag needs to be `true`.
  #[serde(default)]
  pub references_all_functions: bool,
  /// Flag for providing test code lens on `Deno.test` statements.  There is
  /// also the `test_args` setting, but this is not used by the server.
  #[serde(default = "is_true")]
  pub test: bool,
}

impl Default for CodeLensSettings {
  fn default() -> Self {
    Self {
      implementations: false,
      references: false,
      references_all_functions: false,
      test: true,
    }
  }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DenoCompletionSettings {
  #[serde(default)]
  pub imports: ImportCompletionSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClassMemberSnippets {
  #[serde(default = "is_true")]
  pub enabled: bool,
}

impl Default for ClassMemberSnippets {
  fn default() -> Self {
    Self { enabled: true }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLiteralMethodSnippets {
  #[serde(default = "is_true")]
  pub enabled: bool,
}

impl Default for ObjectLiteralMethodSnippets {
  fn default() -> Self {
    Self { enabled: true }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompletionSettings {
  #[serde(default)]
  pub complete_function_calls: bool,
  #[serde(default = "is_true")]
  pub include_automatic_optional_chain_completions: bool,
  #[serde(default = "is_true")]
  pub include_completions_for_import_statements: bool,
  #[serde(default = "is_true")]
  pub names: bool,
  #[serde(default = "is_true")]
  pub paths: bool,
  #[serde(default = "is_true")]
  pub auto_imports: bool,
  #[serde(default = "is_true")]
  pub enabled: bool,
  #[serde(default)]
  pub class_member_snippets: ClassMemberSnippets,
  #[serde(default)]
  pub object_literal_method_snippets: ObjectLiteralMethodSnippets,
}

impl Default for CompletionSettings {
  fn default() -> Self {
    Self {
      complete_function_calls: false,
      include_automatic_optional_chain_completions: true,
      include_completions_for_import_statements: true,
      names: true,
      paths: true,
      auto_imports: true,
      enabled: true,
      class_member_snippets: Default::default(),
      object_literal_method_snippets: Default::default(),
    }
  }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InlayHintsSettings {
  #[serde(default)]
  pub parameter_names: InlayHintsParamNamesOptions,
  #[serde(default)]
  pub parameter_types: InlayHintsParamTypesOptions,
  #[serde(default)]
  pub variable_types: InlayHintsVarTypesOptions,
  #[serde(default)]
  pub property_declaration_types: InlayHintsPropDeclTypesOptions,
  #[serde(default)]
  pub function_like_return_types: InlayHintsFuncLikeReturnTypesOptions,
  #[serde(default)]
  pub enum_member_values: InlayHintsEnumMemberValuesOptions,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InlayHintsParamNamesOptions {
  #[serde(default)]
  pub enabled: InlayHintsParamNamesEnabled,
  #[serde(default = "is_true")]
  pub suppress_when_argument_matches_name: bool,
}

impl Default for InlayHintsParamNamesOptions {
  fn default() -> Self {
    Self {
      enabled: InlayHintsParamNamesEnabled::None,
      suppress_when_argument_matches_name: true,
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InlayHintsParamNamesEnabled {
  None,
  Literals,
  All,
}

impl Default for InlayHintsParamNamesEnabled {
  fn default() -> Self {
    Self::None
  }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InlayHintsParamTypesOptions {
  #[serde(default)]
  pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InlayHintsVarTypesOptions {
  #[serde(default)]
  pub enabled: bool,
  #[serde(default = "is_true")]
  pub suppress_when_type_matches_name: bool,
}

impl Default for InlayHintsVarTypesOptions {
  fn default() -> Self {
    Self {
      enabled: false,
      suppress_when_type_matches_name: true,
    }
  }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InlayHintsPropDeclTypesOptions {
  #[serde(default)]
  pub enabled: bool,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InlayHintsFuncLikeReturnTypesOptions {
  #[serde(default)]
  pub enabled: bool,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InlayHintsEnumMemberValuesOptions {
  #[serde(default)]
  pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportCompletionSettings {
  /// A flag that indicates if non-explicitly set origins should be checked for
  /// supporting import suggestions.
  #[serde(default = "is_true")]
  pub auto_discover: bool,
  /// A map of origins which have had explicitly set if import suggestions are
  /// enabled.
  #[serde(default)]
  pub hosts: HashMap<String, bool>,
}

impl Default for ImportCompletionSettings {
  fn default() -> Self {
    Self {
      auto_discover: true,
      hosts: HashMap::default(),
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TestingSettings {
  /// A vector of arguments which should be used when running the tests for
  /// a workspace.
  #[serde(default)]
  pub args: Vec<String>,
}

impl Default for TestingSettings {
  fn default() -> Self {
    Self {
      args: vec!["--allow-all".to_string(), "--no-check".to_string()],
    }
  }
}

fn default_to_true() -> bool {
  true
}

fn default_document_preload_limit() -> usize {
  1000
}

fn empty_string_none<'de, D: serde::Deserializer<'de>>(
  d: D,
) -> Result<Option<String>, D::Error> {
  let o: Option<String> = Option::deserialize(d)?;
  Ok(o.filter(|s| !s.is_empty()))
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ImportModuleSpecifier {
  NonRelative,
  ProjectRelative,
  Relative,
  Shortest,
}

impl Default for ImportModuleSpecifier {
  fn default() -> Self {
    Self::Shortest
  }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum JsxAttributeCompletionStyle {
  Auto,
  Braces,
  None,
}

impl Default for JsxAttributeCompletionStyle {
  fn default() -> Self {
    Self::Auto
  }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum QuoteStyle {
  Auto,
  Double,
  Single,
}

impl Default for QuoteStyle {
  fn default() -> Self {
    Self::Auto
  }
}

impl From<&FmtOptionsConfig> for QuoteStyle {
  fn from(config: &FmtOptionsConfig) -> Self {
    match config.single_quote {
      Some(true) => QuoteStyle::Single,
      _ => QuoteStyle::Double,
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanguagePreferences {
  #[serde(default)]
  pub import_module_specifier: ImportModuleSpecifier,
  #[serde(default)]
  pub jsx_attribute_completion_style: JsxAttributeCompletionStyle,
  #[serde(default)]
  pub auto_import_file_exclude_patterns: Vec<String>,
  #[serde(default = "is_true")]
  pub use_aliases_for_renames: bool,
  #[serde(default)]
  pub quote_style: QuoteStyle,
  #[serde(default)]
  pub prefer_type_only_auto_imports: bool,
}

impl Default for LanguagePreferences {
  fn default() -> Self {
    LanguagePreferences {
      import_module_specifier: Default::default(),
      jsx_attribute_completion_style: Default::default(),
      auto_import_file_exclude_patterns: vec![],
      use_aliases_for_renames: true,
      quote_style: Default::default(),
      prefer_type_only_auto_imports: false,
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionActionsSettings {
  #[serde(default = "is_true")]
  pub enabled: bool,
}

impl Default for SuggestionActionsSettings {
  fn default() -> Self {
    SuggestionActionsSettings { enabled: true }
  }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateImportsOnFileMoveOptions {
  #[serde(default)]
  pub enabled: UpdateImportsOnFileMoveEnabled,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateImportsOnFileMoveEnabled {
  Always,
  Prompt,
  Never,
}

impl Default for UpdateImportsOnFileMoveEnabled {
  fn default() -> Self {
    Self::Prompt
  }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanguageWorkspaceSettings {
  #[serde(default)]
  pub inlay_hints: InlayHintsSettings,
  #[serde(default)]
  pub preferences: LanguagePreferences,
  #[serde(default)]
  pub suggest: CompletionSettings,
  #[serde(default)]
  pub suggestion_actions: SuggestionActionsSettings,
  #[serde(default)]
  pub update_imports_on_file_move: UpdateImportsOnFileMoveOptions,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum InspectSetting {
  Bool(bool),
  String(String),
}

impl Default for InspectSetting {
  fn default() -> Self {
    InspectSetting::Bool(false)
  }
}

impl InspectSetting {
  pub fn to_address(&self) -> Option<String> {
    match self {
      InspectSetting::Bool(false) => None,
      InspectSetting::Bool(true) => Some("127.0.0.1:9222".to_string()),
      InspectSetting::String(s) => Some(s.clone()),
    }
  }
}

/// Deno language server specific settings that are applied to a workspace.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSettings {
  /// A flag that indicates if Deno is enabled for the workspace.
  pub enable: Option<bool>,

  /// A list of paths, using the root_uri as a base that should be Deno
  /// disabled.
  #[serde(default)]
  pub disable_paths: Vec<String>,

  /// A list of paths, using the root_uri as a base that should be Deno enabled.
  pub enable_paths: Option<Vec<String>>,

  /// An option that points to a path string of the path to utilise as the
  /// cache/DENO_DIR for the language server.
  #[serde(default, deserialize_with = "empty_string_none")]
  pub cache: Option<String>,

  /// Cache local modules and their dependencies on `textDocument/didSave`
  /// notifications corresponding to them.
  #[serde(default)]
  pub cache_on_save: bool,

  /// Override the default stores used to validate certificates. This overrides
  /// the environment variable `DENO_TLS_CA_STORE` if present.
  pub certificate_stores: Option<Vec<String>>,

  /// An option that points to a path string of the config file to apply to
  /// code within the workspace.
  #[serde(default, deserialize_with = "empty_string_none")]
  pub config: Option<String>,

  /// An option that points to a path string of the import map to apply to the
  /// code within the workspace.
  #[serde(default, deserialize_with = "empty_string_none")]
  pub import_map: Option<String>,

  /// Code lens specific settings for the workspace.
  #[serde(default)]
  pub code_lens: CodeLensSettings,

  /// A flag that indicates if internal debug logging should be made available.
  #[serde(default)]
  pub internal_debug: bool,

  #[serde(default)]
  pub internal_inspect: InspectSetting,

  /// Write logs to a file in a project-local directory.
  #[serde(default)]
  pub log_file: bool,

  /// A flag that indicates if linting is enabled for the workspace.
  #[serde(default = "default_to_true")]
  pub lint: bool,

  /// Limits the number of files that can be preloaded by the language server.
  #[serde(default = "default_document_preload_limit")]
  pub document_preload_limit: usize,

  #[serde(default)]
  pub suggest: DenoCompletionSettings,

  /// Testing settings for the workspace.
  #[serde(default)]
  pub testing: TestingSettings,

  /// An option which sets the cert file to use when attempting to fetch remote
  /// resources. This overrides `DENO_CERT` if present.
  #[serde(default, deserialize_with = "empty_string_none")]
  pub tls_certificate: Option<String>,

  /// An option, if set, will unsafely ignore certificate errors when fetching
  /// remote resources.
  #[serde(default)]
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,

  #[serde(default)]
  pub unstable: SafeValue<Vec<String>>,

  #[serde(default)]
  pub javascript: LanguageWorkspaceSettings,

  #[serde(default)]
  pub typescript: LanguageWorkspaceSettings,

  #[serde(default)]
  pub tracing: Option<super::trace::TracingConfigOrEnabled>,
}

impl Default for WorkspaceSettings {
  fn default() -> Self {
    WorkspaceSettings {
      enable: None,
      disable_paths: vec![],
      enable_paths: None,
      cache: None,
      cache_on_save: false,
      certificate_stores: None,
      config: None,
      import_map: None,
      code_lens: Default::default(),
      internal_debug: false,
      internal_inspect: Default::default(),
      log_file: false,
      lint: true,
      document_preload_limit: default_document_preload_limit(),
      suggest: Default::default(),
      testing: Default::default(),
      tls_certificate: None,
      unsafely_ignore_certificate_errors: None,
      unstable: Default::default(),
      javascript: Default::default(),
      typescript: Default::default(),
      tracing: Default::default(),
    }
  }
}

impl WorkspaceSettings {
  pub fn from_raw_settings(
    deno: Value,
    javascript: Value,
    typescript: Value,
  ) -> Self {
    fn parse_or_default<T: Default + DeserializeOwned>(
      value: Value,
      description: &str,
    ) -> T {
      if value.is_null() {
        return T::default();
      }
      match serde_json::from_value(value) {
        Ok(v) => v,
        Err(err) => {
          lsp_warn!("Couldn't parse {description}: {err}");
          T::default()
        }
      }
    }
    let deno_inlay_hints =
      deno.as_object().and_then(|o| o.get("inlayHints").cloned());
    let deno_suggest = deno.as_object().and_then(|o| o.get("suggest").cloned());
    let mut settings: Self = parse_or_default(deno, "settings under \"deno\"");
    settings.javascript =
      parse_or_default(javascript, "settings under \"javascript\"");
    settings.typescript =
      parse_or_default(typescript, "settings under \"typescript\"");
    if let Some(inlay_hints) = deno_inlay_hints {
      let inlay_hints: InlayHintsSettings =
        parse_or_default(inlay_hints, "settings under \"deno.inlayHints\"");
      if inlay_hints.parameter_names.enabled != Default::default() {
        lsp_warn!("\"deno.inlayHints.parameterNames.enabled\" is deprecated. Instead use \"javascript.inlayHints.parameterNames.enabled\" and \"typescript.inlayHints.parameterNames.enabled\".");
        settings.javascript.inlay_hints.parameter_names.enabled =
          inlay_hints.parameter_names.enabled.clone();
        settings.typescript.inlay_hints.parameter_names.enabled =
          inlay_hints.parameter_names.enabled;
      }
      if !inlay_hints
        .parameter_names
        .suppress_when_argument_matches_name
      {
        lsp_warn!("\"deno.inlayHints.parameterNames.suppressWhenArgumentMatchesName\" is deprecated. Instead use \"javascript.inlayHints.parameterNames.suppressWhenArgumentMatchesName\" and \"typescript.inlayHints.parameterNames.suppressWhenArgumentMatchesName\".");
        settings
          .javascript
          .inlay_hints
          .parameter_names
          .suppress_when_argument_matches_name = inlay_hints
          .parameter_names
          .suppress_when_argument_matches_name;
        settings
          .typescript
          .inlay_hints
          .parameter_names
          .suppress_when_argument_matches_name = inlay_hints
          .parameter_names
          .suppress_when_argument_matches_name;
      }
      if inlay_hints.parameter_types.enabled {
        lsp_warn!("\"deno.inlayHints.parameterTypes.enabled\" is deprecated. Instead use \"javascript.inlayHints.parameterTypes.enabled\" and \"typescript.inlayHints.parameterTypes.enabled\".");
        settings.javascript.inlay_hints.parameter_types.enabled =
          inlay_hints.parameter_types.enabled;
        settings.typescript.inlay_hints.parameter_types.enabled =
          inlay_hints.parameter_types.enabled;
      }
      if inlay_hints.variable_types.enabled {
        lsp_warn!("\"deno.inlayHints.variableTypes.enabled\" is deprecated. Instead use \"javascript.inlayHints.variableTypes.enabled\" and \"typescript.inlayHints.variableTypes.enabled\".");
        settings.javascript.inlay_hints.variable_types.enabled =
          inlay_hints.variable_types.enabled;
        settings.typescript.inlay_hints.variable_types.enabled =
          inlay_hints.variable_types.enabled;
      }
      if !inlay_hints.variable_types.suppress_when_type_matches_name {
        lsp_warn!("\"deno.inlayHints.variableTypes.suppressWhenTypeMatchesName\" is deprecated. Instead use \"javascript.inlayHints.variableTypes.suppressWhenTypeMatchesName\" and \"typescript.inlayHints.variableTypes.suppressWhenTypeMatchesName\".");
        settings
          .javascript
          .inlay_hints
          .variable_types
          .suppress_when_type_matches_name =
          inlay_hints.variable_types.suppress_when_type_matches_name;
        settings
          .typescript
          .inlay_hints
          .variable_types
          .suppress_when_type_matches_name =
          inlay_hints.variable_types.suppress_when_type_matches_name;
      }
      if inlay_hints.property_declaration_types.enabled {
        lsp_warn!("\"deno.inlayHints.propertyDeclarationTypes.enabled\" is deprecated. Instead use \"javascript.inlayHints.propertyDeclarationTypes.enabled\" and \"typescript.inlayHints.propertyDeclarationTypes.enabled\".");
        settings
          .javascript
          .inlay_hints
          .property_declaration_types
          .enabled = inlay_hints.property_declaration_types.enabled;
        settings
          .typescript
          .inlay_hints
          .property_declaration_types
          .enabled = inlay_hints.property_declaration_types.enabled;
      }
      if inlay_hints.function_like_return_types.enabled {
        lsp_warn!("\"deno.inlayHints.functionLikeReturnTypes.enabled\" is deprecated. Instead use \"javascript.inlayHints.functionLikeReturnTypes.enabled\" and \"typescript.inlayHints.functionLikeReturnTypes.enabled\".");
        settings
          .javascript
          .inlay_hints
          .function_like_return_types
          .enabled = inlay_hints.function_like_return_types.enabled;
        settings
          .typescript
          .inlay_hints
          .function_like_return_types
          .enabled = inlay_hints.function_like_return_types.enabled;
      }
      if inlay_hints.enum_member_values.enabled {
        lsp_warn!("\"deno.inlayHints.enumMemberValues.enabled\" is deprecated. Instead use \"javascript.inlayHints.enumMemberValues.enabled\" and \"typescript.inlayHints.enumMemberValues.enabled\".");
        settings.javascript.inlay_hints.enum_member_values.enabled =
          inlay_hints.enum_member_values.enabled;
        settings.typescript.inlay_hints.enum_member_values.enabled =
          inlay_hints.enum_member_values.enabled;
      }
    }
    if let Some(suggest) = deno_suggest {
      let suggest: CompletionSettings =
        parse_or_default(suggest, "settings under \"deno.suggest\"");
      if suggest.complete_function_calls {
        lsp_warn!("\"deno.suggest.completeFunctionCalls\" is deprecated. Instead use \"javascript.suggest.completeFunctionCalls\" and \"typescript.suggest.completeFunctionCalls\".");
        settings.javascript.suggest.complete_function_calls =
          suggest.complete_function_calls;
        settings.typescript.suggest.complete_function_calls =
          suggest.complete_function_calls;
      }
      if !suggest.names {
        lsp_warn!("\"deno.suggest.names\" is deprecated. Instead use \"javascript.suggest.names\" and \"typescript.suggest.names\".");
        settings.javascript.suggest.names = suggest.names;
        settings.typescript.suggest.names = suggest.names;
      }
      if !suggest.paths {
        lsp_warn!("\"deno.suggest.paths\" is deprecated. Instead use \"javascript.suggest.paths\" and \"typescript.suggest.paths\".");
        settings.javascript.suggest.paths = suggest.paths;
        settings.typescript.suggest.paths = suggest.paths;
      }
      if !suggest.auto_imports {
        lsp_warn!("\"deno.suggest.autoImports\" is deprecated. Instead use \"javascript.suggest.autoImports\" and \"typescript.suggest.autoImports\".");
        settings.javascript.suggest.auto_imports = suggest.auto_imports;
        settings.typescript.suggest.auto_imports = suggest.auto_imports;
      }
    }
    settings
  }

  pub fn from_initialization_options(options: Value) -> Self {
    let deno = options;
    let javascript = deno
      .as_object()
      .and_then(|o| o.get("javascript").cloned())
      .unwrap_or_default();
    let typescript = deno
      .as_object()
      .and_then(|o| o.get("typescript").cloned())
      .unwrap_or_default();
    Self::from_raw_settings(deno, javascript, typescript)
  }
}

#[derive(Debug, Default, Clone)]
pub struct Settings {
  pub unscoped: Arc<WorkspaceSettings>,
  pub by_workspace_folder: BTreeMap<Arc<Url>, Option<Arc<WorkspaceSettings>>>,
  pub first_folder: Option<Arc<Url>>,
}

impl Settings {
  pub fn path_enabled(&self, path: &Path) -> Option<bool> {
    let (settings, mut folder_uri) = self.get_for_path(path);
    folder_uri = folder_uri.or(self.first_folder.as_ref());
    let mut disable_paths = vec![];
    let mut enable_paths = None;
    if let Some(folder_uri) = folder_uri {
      if let Ok(folder_path) = url_to_file_path(folder_uri) {
        disable_paths = settings
          .disable_paths
          .iter()
          .map(|p| folder_path.join(p))
          .collect::<Vec<_>>();
        enable_paths = settings.enable_paths.as_ref().map(|enable_paths| {
          enable_paths
            .iter()
            .map(|p| folder_path.join(p))
            .collect::<Vec<_>>()
        });
      }
    }

    if disable_paths.iter().any(|p| path.starts_with(p)) {
      Some(false)
    } else if let Some(enable_paths) = &enable_paths {
      for enable_path in enable_paths {
        // Also enable if the checked path is a dir containing an enabled path.
        if path.starts_with(enable_path) || enable_path.starts_with(path) {
          return Some(true);
        }
      }
      Some(false)
    } else {
      settings.enable
    }
  }

  /// Returns `None` if the value should be deferred to the presence of a
  /// `deno.json` file.
  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> Option<bool> {
    let Ok(path) = url_to_file_path(specifier) else {
      // Non-file URLs are not disabled by these settings.
      return Some(true);
    };
    self.path_enabled(&path)
  }

  pub fn get_unscoped(&self) -> &WorkspaceSettings {
    &self.unscoped
  }

  pub fn get_for_path(
    &self,
    path: &Path,
  ) -> (&WorkspaceSettings, Option<&Arc<Url>>) {
    for (folder_uri, settings) in self.by_workspace_folder.iter().rev() {
      if let Some(settings) = settings {
        let Ok(folder_path) = url_to_file_path(folder_uri) else {
          continue;
        };
        if path.starts_with(folder_path) {
          return (settings, Some(folder_uri));
        }
      }
    }
    (&self.unscoped, self.first_folder.as_ref())
  }

  pub fn get_for_uri(
    &self,
    uri: &Uri,
  ) -> (&WorkspaceSettings, Option<&Arc<Url>>) {
    self.get_for_specifier(&uri_to_url(uri))
  }

  pub fn get_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> (&WorkspaceSettings, Option<&Arc<Url>>) {
    let Ok(path) = url_to_file_path(specifier) else {
      return (&self.unscoped, self.first_folder.as_ref());
    };
    self.get_for_path(&path)
  }

  pub fn enable_settings_hash(&self) -> u64 {
    let mut hasher = FastInsecureHasher::new_without_deno_version();
    let unscoped = self.get_unscoped();
    hasher.write_hashable(unscoped.enable);
    hasher.write_hashable(&unscoped.enable_paths);
    hasher.write_hashable(&unscoped.disable_paths);
    hasher.write_hashable(unscoped.document_preload_limit);
    for (folder_uri, settings) in &self.by_workspace_folder {
      hasher.write_hashable(folder_uri);
      hasher.write_hashable(
        settings
          .as_ref()
          .map(|s| (&s.enable, &s.enable_paths, &s.disable_paths)),
      );
    }
    hasher.write_hashable(&self.first_folder);
    hasher.finish()
  }
}

#[derive(Clone, Debug, Default)]
pub struct Config {
  pub client_capabilities: Arc<ClientCapabilities>,
  pub settings: Arc<Settings>,
  pub workspace_folders: Arc<Vec<(Arc<Url>, lsp::WorkspaceFolder)>>,
  pub tree: ConfigTree,
}

impl Config {
  #[cfg(test)]
  pub fn new_with_roots(root_urls: impl IntoIterator<Item = Url>) -> Self {
    use super::urls::url_to_uri;

    let mut config = Self::default();
    let mut folders = vec![];
    for root_url in root_urls {
      let root_uri = url_to_uri(&root_url).unwrap();
      let name = root_url.path_segments().and_then(|s| s.last());
      let name = name.unwrap_or_default().to_string();
      folders.push((
        Arc::new(root_url),
        lsp::WorkspaceFolder {
          uri: root_uri,
          name,
        },
      ));
    }
    config.set_workspace_folders(folders);
    config
  }

  pub fn set_workspace_folders(
    &mut self,
    folders: Vec<(Arc<Url>, lsp::WorkspaceFolder)>,
  ) {
    self.settings = Arc::new(Settings {
      unscoped: self.settings.unscoped.clone(),
      by_workspace_folder: folders
        .iter()
        .map(|(s, _)| (s.clone(), None))
        .collect(),
      first_folder: folders.first().map(|(s, _)| s.clone()),
    });
    self.workspace_folders = Arc::new(folders);
  }

  pub fn set_workspace_settings(
    &mut self,
    unscoped: WorkspaceSettings,
    folder_settings: Vec<(Arc<Url>, WorkspaceSettings)>,
  ) {
    let mut by_folder = folder_settings.into_iter().collect::<HashMap<_, _>>();
    self.settings = Arc::new(Settings {
      unscoped: Arc::new(unscoped),
      by_workspace_folder: self
        .settings
        .by_workspace_folder
        .keys()
        .map(|s| (s.clone(), by_folder.remove(s).map(Arc::new)))
        .collect(),
      first_folder: self.settings.first_folder.clone(),
    });
  }

  pub fn workspace_settings(&self) -> &WorkspaceSettings {
    self.settings.get_unscoped()
  }

  pub fn workspace_settings_for_uri(&self, uri: &Uri) -> &WorkspaceSettings {
    self.settings.get_for_uri(uri).0
  }

  pub fn workspace_settings_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> &WorkspaceSettings {
    self.settings.get_for_specifier(specifier).0
  }

  pub fn language_settings_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&LanguageWorkspaceSettings> {
    let workspace_settings = self.workspace_settings_for_specifier(specifier);
    match MediaType::from_specifier(specifier) {
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::Mjs
      | MediaType::Cjs => Some(&workspace_settings.javascript),
      MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Tsx => Some(&workspace_settings.typescript),
      MediaType::Json
      | MediaType::Wasm
      | MediaType::Css
      | MediaType::Html
      | MediaType::SourceMap
      | MediaType::Sql
      | MediaType::Unknown => None,
    }
  }

  /// Determine if any inlay hints are enabled. This allows short circuiting
  /// when there are no inlay hints enabled.
  pub fn enabled_inlay_hints_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> bool {
    let Some(settings) = self.language_settings_for_specifier(specifier) else {
      return false;
    };
    !matches!(
      settings.inlay_hints.parameter_names.enabled,
      InlayHintsParamNamesEnabled::None
    ) || settings.inlay_hints.parameter_types.enabled
      || settings.inlay_hints.variable_types.enabled
      || settings.inlay_hints.property_declaration_types.enabled
      || settings.inlay_hints.function_like_return_types.enabled
      || settings.inlay_hints.enum_member_values.enabled
  }

  pub fn root_url(&self) -> Option<&Arc<Url>> {
    self.workspace_folders.first().map(|p| &p.0)
  }

  pub fn uri_enabled(&self, uri: &Uri) -> bool {
    if uri.scheme().is_some_and(|s| s.eq_lowercase("deno")) {
      return true;
    }
    self.specifier_enabled(&uri_to_url(uri))
  }

  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> bool {
    if self.tree.in_global_npm_cache(specifier) {
      return true;
    }
    let data = self.tree.data_for_specifier(specifier);
    if let Some(data) = &data {
      if let Ok(path) = specifier.to_file_path() {
        // deno_config's exclusion checks exclude vendor dirs invariably. We
        // don't want that behavior here.
        if data.exclude_files.matches_path(&path)
          && !data
            .vendor_dir
            .as_ref()
            .is_some_and(|p| path.starts_with(p))
        {
          return false;
        }
      }
    }
    self
      .settings
      .specifier_enabled(specifier)
      .unwrap_or_else(|| data.and_then(|d| d.maybe_deno_json()).is_some())
  }

  pub fn specifier_enabled_for_test(
    &self,
    specifier: &ModuleSpecifier,
  ) -> bool {
    if let Some(data) = self.tree.data_for_specifier(specifier) {
      if !data.test_config.files.matches_specifier(specifier) {
        return false;
      }
    }
    self.specifier_enabled(specifier)
  }

  pub fn log_file(&self) -> bool {
    self.settings.unscoped.log_file
  }

  pub fn internal_inspect(&self) -> &InspectSetting {
    &self.settings.unscoped.internal_inspect
  }

  pub fn set_client_capabilities(
    &mut self,
    client_capabilities: ClientCapabilities,
  ) {
    self.client_capabilities = Arc::new(client_capabilities);
  }

  pub fn workspace_capable(&self) -> bool {
    self.client_capabilities.workspace.is_some()
  }

  pub fn workspace_configuration_capable(&self) -> bool {
    (|| self.client_capabilities.workspace.as_ref()?.configuration)()
      .unwrap_or(false)
  }

  pub fn did_change_watched_files_capable(&self) -> bool {
    (|| {
      let workspace = self.client_capabilities.workspace.as_ref()?;
      let did_change_watched_files =
        workspace.did_change_watched_files.as_ref()?;
      did_change_watched_files.dynamic_registration
    })()
    .unwrap_or(false)
  }

  pub fn will_rename_files_capable(&self) -> bool {
    (|| {
      let workspace = self.client_capabilities.workspace.as_ref()?;
      let file_operations = workspace.file_operations.as_ref()?;
      file_operations.dynamic_registration.filter(|d| *d)?;
      file_operations.will_rename
    })()
    .unwrap_or(false)
  }

  pub fn line_folding_only_capable(&self) -> bool {
    (|| {
      let text_document = self.client_capabilities.text_document.as_ref()?;
      text_document.folding_range.as_ref()?.line_folding_only
    })()
    .unwrap_or(false)
  }

  pub fn code_action_disabled_capable(&self) -> bool {
    (|| {
      let text_document = self.client_capabilities.text_document.as_ref()?;
      text_document.code_action.as_ref()?.disabled_support
    })()
    .unwrap_or(false)
  }

  pub fn snippet_support_capable(&self) -> bool {
    (|| {
      let text_document = self.client_capabilities.text_document.as_ref()?;
      let completion = text_document.completion.as_ref()?;
      completion.completion_item.as_ref()?.snippet_support
    })()
    .unwrap_or(false)
  }

  pub fn testing_api_capable(&self) -> bool {
    (|| {
      let experimental = self.client_capabilities.experimental.as_ref()?;
      experimental.get("testingApi")?.as_bool()
    })()
    .unwrap_or(false)
  }
}

#[derive(Debug, Serialize)]
pub struct LspTsConfig {
  #[serde(flatten)]
  inner: TsConfig,
}

impl Default for LspTsConfig {
  fn default() -> Self {
    Self {
      inner: TsConfig::new(json!({
        "allowJs": true,
        "esModuleInterop": true,
        "experimentalDecorators": false,
        "isolatedModules": true,
        "lib": ["deno.ns", "deno.window", "deno.unstable"],
        "module": "esnext",
        "moduleDetection": "force",
        "noEmit": true,
        "noImplicitOverride": true,
        "resolveJsonModule": true,
        "strict": true,
        "target": "esnext",
        "useDefineForClassFields": true,
        "jsx": "react",
        "jsxFactory": "React.createElement",
        "jsxFragmentFactory": "React.Fragment",
      })),
    }
  }
}

impl LspTsConfig {
  pub fn new(raw_ts_config: TsConfigWithIgnoredOptions) -> Self {
    let mut base_ts_config = Self::default();
    for ignored_options in &raw_ts_config.ignored_options {
      lsp_warn!("{}", ignored_options)
    }
    base_ts_config.inner.merge_mut(raw_ts_config.ts_config);
    base_ts_config
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigWatchedFileType {
  DenoJson,
  Lockfile,
  NpmRc,
  PackageJson,
  ImportMap,
}

/// Contains the config file and dependent information.
#[derive(Debug, Clone)]
pub struct ConfigData {
  pub scope: Arc<ModuleSpecifier>,
  pub canonicalized_scope: Option<Arc<ModuleSpecifier>>,
  pub member_dir: Arc<WorkspaceDirectory>,
  pub fmt_config: Arc<FmtConfig>,
  pub lint_config: Arc<WorkspaceDirLintConfig>,
  pub test_config: Arc<TestConfig>,
  pub exclude_files: Arc<PathOrPatternSet>,
  pub linter: Arc<CliLinter>,
  pub ts_config: Arc<LspTsConfig>,
  pub byonm: bool,
  pub node_modules_dir: Option<PathBuf>,
  pub vendor_dir: Option<PathBuf>,
  pub lockfile: Option<Arc<CliLockfile>>,
  pub npmrc: Option<Arc<ResolvedNpmRc>>,
  pub resolver: Arc<WorkspaceResolver<CliSys>>,
  pub import_map_from_settings: Option<ModuleSpecifier>,
  pub unstable: BTreeSet<String>,
  watched_files: HashMap<ModuleSpecifier, ConfigWatchedFileType>,
}

impl ConfigData {
  #[allow(clippy::too_many_arguments)]
  async fn load(
    specified_config: Option<&Path>,
    scope: &Arc<Url>,
    settings: &Settings,
    file_fetcher: &Arc<CliFileFetcher>,
    // sync requirement is because the lsp requires sync
    deno_json_cache: &(dyn DenoJsonCache + Sync),
    pkg_json_cache: &(dyn PackageJsonCache + Sync),
    workspace_cache: &(dyn WorkspaceCache + Sync),
    lockfile_package_info_provider: &Arc<
      dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync,
    >,
  ) -> Self {
    let scope = scope.clone();
    let discover_result = match scope.to_file_path() {
      Ok(scope_dir_path) => {
        let paths = [scope_dir_path];
        WorkspaceDirectory::discover(
          &CliSys::default(),
          match specified_config {
            Some(config_path) => {
              deno_config::workspace::WorkspaceDiscoverStart::ConfigFile(
                config_path,
              )
            }
            None => {
              deno_config::workspace::WorkspaceDiscoverStart::Paths(&paths)
            }
          },
          &WorkspaceDiscoverOptions {
            additional_config_file_names: &[],
            deno_json_cache: Some(deno_json_cache),
            pkg_json_cache: Some(pkg_json_cache),
            workspace_cache: Some(workspace_cache),
            discover_pkg_json: !has_flag_env_var("DENO_NO_PACKAGE_JSON"),
            maybe_vendor_override: None,
          },
        )
        .map(Arc::new)
        .map_err(AnyError::from)
      }
      Err(()) => Err(anyhow!("Scope '{}' was not a directory path.", scope)),
    };
    match discover_result {
      Ok(member_dir) => {
        Self::load_inner(
          member_dir,
          scope,
          settings,
          Some(file_fetcher),
          lockfile_package_info_provider,
        )
        .await
      }
      Err(err) => {
        lsp_warn!("  Couldn't open workspace \"{}\": {}", scope.as_str(), err);
        let member_dir =
          Arc::new(WorkspaceDirectory::empty(WorkspaceDirectoryEmptyOptions {
            root_dir: scope.clone(),
            use_vendor_dir: VendorEnablement::Disable,
          }));
        let mut data = Self::load_inner(
          member_dir,
          scope.clone(),
          settings,
          Some(file_fetcher),
          lockfile_package_info_provider,
        )
        .await;
        // check if any of these need to be added to the workspace
        let files = [
          (
            scope.join("deno.json").unwrap(),
            ConfigWatchedFileType::DenoJson,
          ),
          (
            scope.join("deno.jsonc").unwrap(),
            ConfigWatchedFileType::DenoJson,
          ),
          (
            scope.join("package.json").unwrap(),
            ConfigWatchedFileType::PackageJson,
          ),
        ];
        for (url, file_type) in files {
          let Some(file_path) = url.to_file_path().ok() else {
            continue;
          };
          if file_path.exists() {
            data.watched_files.insert(url.clone(), file_type);
            let canonicalized_specifier =
              canonicalize_path_maybe_not_exists(&file_path)
                .ok()
                .and_then(|p| ModuleSpecifier::from_file_path(p).ok());
            if let Some(specifier) = canonicalized_specifier {
              data.watched_files.insert(specifier, file_type);
            }
          }
        }
        data
      }
    }
  }

  async fn load_inner(
    member_dir: Arc<WorkspaceDirectory>,
    scope: Arc<ModuleSpecifier>,
    settings: &Settings,
    file_fetcher: Option<&Arc<CliFileFetcher>>,
    lockfile_package_info_provider: &Arc<
      dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync,
    >,
  ) -> Self {
    let (settings, workspace_folder) = settings.get_for_specifier(&scope);
    let mut watched_files = HashMap::with_capacity(10);
    let mut add_watched_file =
      |specifier: ModuleSpecifier, file_type: ConfigWatchedFileType| {
        let maybe_canonicalized = specifier
          .to_file_path()
          .ok()
          .and_then(|p| canonicalize_path_maybe_not_exists(&p).ok())
          .and_then(|p| ModuleSpecifier::from_file_path(p).ok());
        if let Some(canonicalized) = maybe_canonicalized {
          if canonicalized != specifier {
            watched_files.entry(canonicalized).or_insert(file_type);
          }
        }
        watched_files.entry(specifier).or_insert(file_type);
      };

    let canonicalized_scope = (|| {
      let path = scope.to_file_path().ok()?;
      let path = canonicalize_path_maybe_not_exists(&path).ok()?;
      let specifier = ModuleSpecifier::from_directory_path(path).ok()?;
      if specifier == *scope {
        return None;
      }
      Some(Arc::new(specifier))
    })();

    if let Some(deno_json) = member_dir.maybe_deno_json() {
      lsp_log!(
        "  Resolved Deno configuration file: \"{}\"",
        deno_json.specifier
      );

      add_watched_file(
        deno_json.specifier.clone(),
        ConfigWatchedFileType::DenoJson,
      );
    }

    if let Some(pkg_json) = member_dir.maybe_pkg_json() {
      lsp_log!("  Resolved package.json: \"{}\"", pkg_json.specifier());

      add_watched_file(
        pkg_json.specifier(),
        ConfigWatchedFileType::PackageJson,
      );
    }

    // todo(dsherret): cache this so we don't load this so many times
    let npmrc =
      discover_npmrc_from_workspace(&CliSys::default(), &member_dir.workspace)
        .inspect(|(_, path)| {
          if let Some(path) = path {
            lsp_log!("  Resolved .npmrc: \"{}\"", path.display());

            if let Ok(specifier) = ModuleSpecifier::from_file_path(path) {
              add_watched_file(specifier, ConfigWatchedFileType::NpmRc);
            }
          }
        })
        .inspect_err(|err| {
          lsp_warn!("  Couldn't read .npmrc for \"{scope}\": {err}");
        })
        .map(|(r, _)| Arc::new(r))
        .ok();
    let default_file_pattern_base =
      scope.to_file_path().unwrap_or_else(|_| PathBuf::from("/"));
    let fmt_config = Arc::new(
      member_dir
        .to_fmt_config(FilePatterns::new_with_base(member_dir.dir_path()))
        .inspect_err(|err| {
          lsp_warn!("  Couldn't read formatter configuration: {}", err)
        })
        .ok()
        .unwrap_or_else(|| {
          FmtConfig::new_with_base(default_file_pattern_base.clone())
        }),
    );
    let lint_config = Arc::new(
      member_dir
        .to_lint_config(FilePatterns::new_with_base(member_dir.dir_path()))
        .inspect_err(|err| {
          lsp_warn!("  Couldn't read lint configuration: {}", err)
        })
        .ok()
        .unwrap_or_else(|| WorkspaceDirLintConfig {
          rules: Default::default(),
          plugins: Default::default(),
          files: FilePatterns::new_with_base(default_file_pattern_base.clone()),
        }),
    );

    let test_config = Arc::new(
      member_dir
        .to_test_config(FilePatterns::new_with_base(member_dir.dir_path()))
        .inspect_err(|err| {
          lsp_warn!("  Couldn't read test configuration: {}", err)
        })
        .ok()
        .unwrap_or_else(|| {
          TestConfig::new_with_base(default_file_pattern_base.clone())
        }),
    );
    let exclude_files = Arc::new(
      member_dir
        .workspace
        .resolve_config_excludes()
        .inspect_err(|err| {
          lsp_warn!("  Couldn't read config excludes: {}", err)
        })
        .ok()
        .unwrap_or_default(),
    );

    let ts_config = member_dir
      .to_raw_user_provided_tsconfig()
      .map(LspTsConfig::new)
      .unwrap_or_default();

    let deno_lint_config =
      if ts_config.inner.0.get("jsx").and_then(|v| v.as_str()) == Some("react")
      {
        let default_jsx_factory =
          ts_config.inner.0.get("jsxFactory").and_then(|v| v.as_str());
        let default_jsx_fragment_factory = ts_config
          .inner
          .0
          .get("jsxFragmentFactory")
          .and_then(|v| v.as_str());
        DenoLintConfig {
          default_jsx_factory: default_jsx_factory.map(String::from),
          default_jsx_fragment_factory: default_jsx_fragment_factory
            .map(String::from),
        }
      } else {
        DenoLintConfig {
          default_jsx_factory: None,
          default_jsx_fragment_factory: None,
        }
      };

    let vendor_dir = member_dir.workspace.vendor_dir_path().cloned();
    // todo(dsherret): add caching so we don't load this so many times
    let lockfile = resolve_lockfile_from_workspace(
      &member_dir,
      lockfile_package_info_provider,
    )
    .await
    .map(Arc::new);
    if let Some(lockfile) = &lockfile {
      if let Ok(specifier) = ModuleSpecifier::from_file_path(&lockfile.filename)
      {
        add_watched_file(specifier, ConfigWatchedFileType::Lockfile);
      }
    }

    let node_modules_dir =
      member_dir.workspace.node_modules_dir().unwrap_or_default();
    let byonm = match node_modules_dir {
      Some(mode) => mode == NodeModulesDirMode::Manual,
      None => member_dir.workspace.root_pkg_json().is_some(),
    };
    if byonm {
      lsp_log!("  Enabled 'bring your own node_modules'.");
    }
    let node_modules_dir =
      resolve_node_modules_dir(&member_dir.workspace, byonm);

    // Mark the import map as a watched file
    if let Some(import_map_specifier) = member_dir
      .workspace
      .to_import_map_path()
      .ok()
      .flatten()
      .and_then(|path| Url::from_file_path(path).ok())
    {
      add_watched_file(
        import_map_specifier.clone(),
        ConfigWatchedFileType::ImportMap,
      );
    }
    // attempt to create a resolver for the workspace
    let pkg_json_dep_resolution = if byonm {
      PackageJsonDepResolution::Disabled
    } else {
      // todo(dsherret): this should be false for nodeModulesDir: true
      PackageJsonDepResolution::Enabled
    };
    let mut import_map_from_settings = {
      let is_config_import_map = member_dir
        .maybe_deno_json()
        .map(|c| c.is_an_import_map() || c.json.import_map.is_some())
        .or_else(|| {
          member_dir
            .workspace
            .root_deno_json()
            .map(|c| c.is_an_import_map() || c.json.import_map.is_some())
        })
        .unwrap_or(false);
      if is_config_import_map {
        None
      } else {
        settings.import_map.as_ref().and_then(|import_map_str| {
          Url::parse(import_map_str)
            .ok()
            .or_else(|| workspace_folder?.join(import_map_str).ok())
        })
      }
    };

    let specified_import_map = {
      let is_config_import_map = member_dir
        .maybe_deno_json()
        .map(|c| c.is_an_import_map() || c.json.import_map.is_some())
        .or_else(|| {
          member_dir
            .workspace
            .root_deno_json()
            .map(|c| c.is_an_import_map() || c.json.import_map.is_some())
        })
        .unwrap_or(false);
      if is_config_import_map {
        import_map_from_settings = None;
      }
      if let Some(import_map_url) = &import_map_from_settings {
        add_watched_file(
          import_map_url.clone(),
          ConfigWatchedFileType::ImportMap,
        );
        let fetch_result = file_fetcher
          .as_ref()
          .unwrap()
          .fetch_bypass_permissions(import_map_url)
          .await;

        let value_result = fetch_result.and_then(|f| {
          serde_json::from_slice::<Value>(&f.source).map_err(|e| e.into())
        });
        match value_result {
          Ok(value) => Some(SpecifiedImportMap {
            base_url: import_map_url.clone(),
            value,
          }),
          Err(err) => {
            lsp_warn!(
              "  Couldn't read import map \"{}\": {}",
              import_map_url.as_str(),
              err
            );
            import_map_from_settings = None;
            None
          }
        }
      } else {
        None
      }
    };
    let unstable = member_dir
      .workspace
      .unstable_features()
      .iter()
      .chain(settings.unstable.as_deref())
      .cloned()
      .collect::<BTreeSet<_>>();
    let unstable_sloppy_imports = std::env::var("DENO_UNSTABLE_SLOPPY_IMPORTS")
      .is_ok()
      || unstable.contains("sloppy-imports");
    let resolver = WorkspaceResolver::from_workspace(
      &member_dir.workspace,
      CliSys::default(),
      CreateResolverOptions {
        pkg_json_dep_resolution,
        specified_import_map,
        sloppy_imports_options: if unstable_sloppy_imports {
          SloppyImportsOptions::Enabled
        } else {
          SloppyImportsOptions::Disabled
        },
        fs_cache_options: FsCacheOptions::Disabled,
      },
    )
    .inspect_err(|err| {
      lsp_warn!(
        "  Failed to load resolver: {}",
        err // will contain the specifier
      );
    })
    .ok()
    .unwrap_or_else(|| {
      // create a dummy resolver
      WorkspaceResolver::new_raw(
        scope.clone(),
        None,
        member_dir.workspace.resolver_jsr_pkgs().collect(),
        member_dir.workspace.package_jsons().cloned().collect(),
        pkg_json_dep_resolution,
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        CliSys::default(),
      )
    });
    if !resolver.diagnostics().is_empty() {
      lsp_warn!(
        "  Resolver diagnostics:\n{}",
        resolver
          .diagnostics()
          .iter()
          .map(|d| format!("    - {d}"))
          .collect::<Vec<_>>()
          .join("\n")
      );
    }
    let resolver = Arc::new(resolver);
    let lint_rule_provider = LintRuleProvider::new(Some(resolver.clone()));

    let lint_options =
      LintOptions::resolve((*lint_config).clone(), &LintFlags::default())
        .inspect_err(|err| {
          lsp_warn!("  Failed to resolve linter options: {}", err)
        })
        .ok()
        .unwrap_or_default();
    let mut plugin_runner = None;
    if !lint_options.plugins.is_empty() {
      fn logger_printer(msg: &str, _is_err: bool) {
        lsp_log!("pluggin runner - {}", msg);
      }
      let logger = crate::tools::lint::PluginLogger::new(logger_printer);
      let plugin_load_result =
        crate::tools::lint::create_runner_and_load_plugins(
          lint_options.plugins.clone(),
          logger,
          lint_options.rules.exclude.clone(),
        )
        .await;
      match plugin_load_result {
        Ok(runner) => {
          plugin_runner = Some(Arc::new(runner));
        }
        Err(err) => {
          lsp_warn!("Failed to load lint plugins: {}", err);
        }
      }
    }

    let linter = Arc::new(CliLinter::new(CliLinterOptions {
      configured_rules: lint_rule_provider.resolve_lint_rules(
        lint_options.rules,
        member_dir.maybe_deno_json().map(|c| c.as_ref()),
      ),
      fix: false,
      deno_lint_config,
      maybe_plugin_runner: plugin_runner,
    }));

    ConfigData {
      scope,
      canonicalized_scope,
      member_dir,
      resolver,
      fmt_config,
      lint_config,
      test_config,
      linter,
      exclude_files,
      ts_config: Arc::new(ts_config),
      byonm,
      node_modules_dir,
      vendor_dir,
      lockfile,
      npmrc,
      import_map_from_settings,
      unstable,
      watched_files,
    }
  }

  pub fn maybe_deno_json(
    &self,
  ) -> Option<&Arc<deno_config::deno_json::ConfigFile>> {
    self.member_dir.maybe_deno_json()
  }

  pub fn maybe_pkg_json(&self) -> Option<&Arc<deno_package_json::PackageJson>> {
    self.member_dir.maybe_pkg_json()
  }

  pub fn maybe_jsx_import_source_config(
    &self,
  ) -> Option<JsxImportSourceConfig> {
    self
      .member_dir
      .to_maybe_jsx_import_source_config()
      .ok()
      .flatten()
  }

  pub fn scope_contains_specifier(&self, specifier: &ModuleSpecifier) -> bool {
    specifier.as_str().starts_with(self.scope.as_str())
      || self
        .canonicalized_scope
        .as_ref()
        .map(|s| specifier.as_str().starts_with(s.as_str()))
        .unwrap_or(false)
  }
}

#[derive(Clone, Debug, Default)]
pub struct ConfigTree {
  scopes: Arc<BTreeMap<Arc<Url>, Arc<ConfigData>>>,
  global_npm_cache_url: Option<Arc<Url>>,
}

impl ConfigTree {
  pub fn scope_for_specifier(&self, specifier: &Url) -> Option<&Arc<Url>> {
    self
      .scopes
      .iter()
      .rfind(|(_, d)| d.scope_contains_specifier(specifier))
      .map(|(s, _)| s)
  }

  pub fn data_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&Arc<ConfigData>> {
    self
      .scope_for_specifier(specifier)
      .and_then(|s| self.scopes.get(s))
  }

  pub fn data_by_scope(&self) -> &Arc<BTreeMap<Arc<Url>, Arc<ConfigData>>> {
    &self.scopes
  }

  pub fn workspace_dir_for_specifier(
    &self,
    specifier: &Url,
  ) -> Option<&WorkspaceDirectory> {
    self
      .data_for_specifier(specifier)
      .map(|d| d.member_dir.as_ref())
  }

  pub fn config_files(&self) -> Vec<&Arc<ConfigFile>> {
    self
      .scopes
      .iter()
      .filter_map(|(_, d)| d.maybe_deno_json())
      .collect()
  }

  pub fn package_jsons(&self) -> Vec<&Arc<PackageJson>> {
    self
      .scopes
      .iter()
      .filter_map(|(_, d)| d.maybe_pkg_json())
      .collect()
  }

  pub fn fmt_config_for_specifier(&self, specifier: &Url) -> Arc<FmtConfig> {
    self
      .data_for_specifier(specifier)
      .map(|d| d.fmt_config.clone())
      .unwrap_or_else(|| Arc::new(FmtConfig::new_with_base(PathBuf::from("/"))))
  }

  /// Returns (scope_url, type).
  pub fn watched_file_type(
    &self,
    specifier: &Url,
  ) -> Option<(&Arc<Url>, ConfigWatchedFileType)> {
    for (scope_url, data) in self.scopes.iter() {
      if let Some(typ) = data.watched_files.get(specifier) {
        return Some((scope_url, *typ));
      }
    }
    None
  }

  pub fn is_watched_file(&self, specifier: &Url) -> bool {
    let path = specifier.path();
    if path.ends_with("/deno.json")
      || path.ends_with("/deno.jsonc")
      || path.ends_with("/package.json")
      || path.ends_with("/node_modules/.package-lock.json")
      || path.ends_with("/node_modules/.yarn-integrity.json")
      || path.ends_with("/node_modules/.modules.yaml")
      || path.ends_with("/node_modules/.deno/.setup-cache.bin")
    {
      return true;
    }
    self
      .scopes
      .values()
      .any(|data| data.watched_files.contains_key(specifier))
  }

  pub fn to_did_refresh_params(
    &self,
  ) -> lsp_custom::DidRefreshDenoConfigurationTreeNotificationParams {
    let data = self
      .scopes
      .values()
      .filter_map(|data| {
        let workspace_root_scope_uri =
          Some(data.member_dir.workspace.root_dir())
            .filter(|s| *s != data.member_dir.dir_url())
            .and_then(|s| url_to_uri(s).ok());
        Some(lsp_custom::DenoConfigurationData {
          scope_uri: url_to_uri(&data.scope).ok()?,
          deno_json: data.maybe_deno_json().and_then(|c| {
            if workspace_root_scope_uri.is_some()
              && Some(&c.specifier)
                == data
                  .member_dir
                  .workspace
                  .root_deno_json()
                  .map(|c| &c.specifier)
            {
              return None;
            }
            Some(lsp::TextDocumentIdentifier {
              uri: url_to_uri(&c.specifier).ok()?,
            })
          }),
          package_json: data.maybe_pkg_json().and_then(|p| {
            Some(lsp::TextDocumentIdentifier {
              uri: url_to_uri(&p.specifier()).ok()?,
            })
          }),
          workspace_root_scope_uri,
        })
      })
      .collect();
    let deno_dir_npm_folder_uri = self
      .global_npm_cache_url
      .as_ref()
      .and_then(|s| url_to_uri(s).ok());
    lsp_custom::DidRefreshDenoConfigurationTreeNotificationParams {
      data,
      deno_dir_npm_folder_uri,
    }
  }

  pub fn in_global_npm_cache(&self, url: &Url) -> bool {
    self
      .global_npm_cache_url
      .as_ref()
      .is_some_and(|s| url.as_str().starts_with(s.as_str()))
  }

  pub async fn refresh(
    &mut self,
    settings: &Settings,
    workspace_files: &IndexSet<PathBuf>,
    file_fetcher: &Arc<CliFileFetcher>,
    deno_dir: &DenoDir,
    lockfile_package_info_provider: &Arc<
      dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync,
    >,
  ) {
    lsp_log!("Refreshing configuration tree...");
    // since we're resolving a workspace multiple times in different
    // folders, we want to cache all the lookups and config files across
    // ConfigData::load calls
    let deno_json_cache = DenoJsonMemCache::default();
    let pkg_json_cache = PackageJsonMemCache::default();
    let workspace_cache = WorkspaceMemCache::default();
    let mut scopes = BTreeMap::new();
    for (folder_url, ws_settings) in &settings.by_workspace_folder {
      let mut ws_settings = ws_settings.as_ref();
      if Some(folder_url) == settings.first_folder.as_ref() {
        ws_settings = ws_settings.or(Some(&settings.unscoped));
      }
      if let Some(ws_settings) = ws_settings {
        let config_file_path = (|| {
          let config_setting = ws_settings.config.as_ref()?;
          let config_uri = folder_url.join(config_setting).ok()?;
          url_to_file_path(&config_uri).ok()
        })();
        if config_file_path.is_some() || ws_settings.import_map.is_some() {
          scopes.insert(
            folder_url.clone(),
            Arc::new(
              ConfigData::load(
                config_file_path.as_deref(),
                folder_url,
                settings,
                file_fetcher,
                &deno_json_cache,
                &pkg_json_cache,
                &workspace_cache,
                lockfile_package_info_provider,
              )
              .await,
            ),
          );
        }
      }
    }

    for path in workspace_files {
      let Ok(file_url) = Url::from_file_path(path) else {
        continue;
      };
      if !(file_url.path().ends_with("/deno.json")
        || file_url.path().ends_with("/deno.jsonc")
        || file_url.path().ends_with("/package.json"))
      {
        continue;
      }
      let Ok(scope) = file_url.join(".").map(Arc::new) else {
        continue;
      };
      if scopes.contains_key(&scope) {
        continue;
      }
      let data = Arc::new(
        ConfigData::load(
          None,
          &scope,
          settings,
          file_fetcher,
          &deno_json_cache,
          &pkg_json_cache,
          &workspace_cache,
          lockfile_package_info_provider,
        )
        .await,
      );
      scopes.insert(scope, data.clone());
      for (member_scope, _) in data.member_dir.workspace.config_folders() {
        if scopes.contains_key(member_scope) {
          continue;
        }
        let member_data = ConfigData::load(
          None,
          member_scope,
          settings,
          file_fetcher,
          &deno_json_cache,
          &pkg_json_cache,
          &workspace_cache,
          lockfile_package_info_provider,
        )
        .await;
        scopes.insert(member_scope.clone(), Arc::new(member_data));
      }
    }

    self.scopes = Arc::new(scopes);
    self.global_npm_cache_url =
      Url::from_directory_path(deno_dir.npm_folder_path())
        .ok()
        .map(Arc::new);
  }

  #[cfg(test)]
  pub async fn inject_config_file(
    &mut self,
    config_file: ConfigFile,
    lockfile_package_info_provider: &Arc<
      dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync,
    >,
  ) {
    use sys_traits::FsCreateDirAll;
    use sys_traits::FsWrite;

    let scope = Arc::new(config_file.specifier.join(".").unwrap());
    let json_text = serde_json::to_string(&config_file.json).unwrap();
    let memory_sys = sys_traits::impls::InMemorySys::default();
    let config_path = url_to_file_path(&config_file.specifier).unwrap();
    memory_sys
      .fs_create_dir_all(config_path.parent().unwrap())
      .unwrap();
    memory_sys.fs_write(&config_path, json_text).unwrap();
    let workspace_dir = Arc::new(
      WorkspaceDirectory::discover(
        &memory_sys,
        deno_config::workspace::WorkspaceDiscoverStart::ConfigFile(
          &config_path,
        ),
        &deno_config::workspace::WorkspaceDiscoverOptions {
          ..Default::default()
        },
      )
      .unwrap(),
    );
    let data = Arc::new(
      ConfigData::load_inner(
        workspace_dir,
        scope.clone(),
        &Default::default(),
        None,
        lockfile_package_info_provider,
      )
      .await,
    );
    assert!(data.maybe_deno_json().is_some());
    self.scopes = Arc::new([(scope, data)].into_iter().collect());
  }
}

async fn resolve_lockfile_from_workspace(
  workspace: &WorkspaceDirectory,
  lockfile_package_info_provider: &Arc<
    dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync,
  >,
) -> Option<CliLockfile> {
  let lockfile_path = match workspace.workspace.resolve_lockfile_path() {
    Ok(Some(value)) => value,
    Ok(None) => return None,
    Err(err) => {
      lsp_warn!("Error resolving lockfile: {:#}", err);
      return None;
    }
  };
  let frozen = workspace
    .workspace
    .root_deno_json()
    .and_then(|c| c.to_lock_config().ok().flatten().map(|c| c.frozen()))
    .unwrap_or(false);
  resolve_lockfile_from_path(
    lockfile_path,
    frozen,
    lockfile_package_info_provider,
  )
  .await
}

fn resolve_node_modules_dir(
  workspace: &Workspace,
  byonm: bool,
) -> Option<PathBuf> {
  // For the language server, require an explicit opt-in via the
  // `nodeModulesDir: true` setting in the deno.json file. This is to
  // reduce the chance of modifying someone's node_modules directory
  // without them having asked us to do so.
  let node_modules_mode = workspace.node_modules_dir().ok().flatten();
  let explicitly_disabled = node_modules_mode == Some(NodeModulesDirMode::None);
  if explicitly_disabled {
    return None;
  }
  let enabled = byonm
    || node_modules_mode
      .map(|m| m.uses_node_modules_dir())
      .unwrap_or(false)
    || workspace.vendor_dir_path().is_some();

  if !enabled {
    return None;
  }
  let node_modules_dir = workspace
    .root_dir()
    .to_file_path()
    .ok()?
    .join("node_modules");
  canonicalize_path_maybe_not_exists(&node_modules_dir).ok()
}

async fn resolve_lockfile_from_path(
  lockfile_path: PathBuf,
  frozen: bool,
  lockfile_package_info_provider: &Arc<
    dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync,
  >,
) -> Option<CliLockfile> {
  match CliLockfile::read_from_path(
    &CliSys::default(),
    CliLockfileReadFromPathOptions {
      file_path: lockfile_path,
      frozen,
      skip_write: false,
    },
    &**lockfile_package_info_provider,
  )
  .await
  {
    Ok(value) => {
      if value.filename.exists() {
        if let Ok(specifier) = ModuleSpecifier::from_file_path(&value.filename)
        {
          lsp_log!("  Resolved lockfile: \"{}\"", specifier);
        }
      }
      Some(value)
    }
    Err(err) => {
      lsp_warn!("Error loading lockfile: {:#}", err);
      None
    }
  }
}

// todo(dsherret): switch to RefCell once the lsp no longer requires Sync
#[derive(Default)]
struct DenoJsonMemCache(Mutex<HashMap<PathBuf, Arc<ConfigFile>>>);

impl deno_config::deno_json::DenoJsonCache for DenoJsonMemCache {
  fn get(&self, path: &Path) -> Option<Arc<ConfigFile>> {
    self.0.lock().get(path).cloned()
  }

  fn set(&self, path: PathBuf, data: Arc<ConfigFile>) {
    self.0.lock().insert(path, data);
  }
}

#[derive(Debug, Default)]
struct PackageJsonMemCache(Mutex<HashMap<PathBuf, Arc<PackageJson>>>);

impl deno_package_json::PackageJsonCache for PackageJsonMemCache {
  fn get(&self, path: &Path) -> Option<Arc<PackageJson>> {
    self.0.lock().get(path).cloned()
  }

  fn set(&self, path: PathBuf, data: Arc<PackageJson>) {
    self.0.lock().insert(path, data);
  }
}

#[derive(Default)]
struct WorkspaceMemCache(Mutex<HashMap<PathBuf, Arc<Workspace>>>);

impl deno_config::workspace::WorkspaceCache for WorkspaceMemCache {
  fn get(&self, dir_path: &Path) -> Option<Arc<Workspace>> {
    self.0.lock().get(dir_path).cloned()
  }

  fn set(&self, dir_path: PathBuf, workspace: Arc<Workspace>) {
    self.0.lock().insert(dir_path, workspace);
  }
}

#[cfg(test)]
mod tests {
  use deno_core::resolve_url;
  use deno_core::serde_json;
  use deno_core::serde_json::json;
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn test_config_specifier_enabled() {
    let root_uri = resolve_url("file:///").unwrap();
    let mut config = Config::new_with_roots(vec![root_uri]);
    let specifier = resolve_url("file:///a.ts").unwrap();
    assert!(!config.specifier_enabled(&specifier));
    config.set_workspace_settings(
      serde_json::from_value(json!({
        "enable": true
      }))
      .unwrap(),
      vec![],
    );
    assert!(config.specifier_enabled(&specifier));
  }

  #[test]
  fn test_config_snapshot_specifier_enabled() {
    let root_uri = resolve_url("file:///").unwrap();
    let mut config = Config::new_with_roots(vec![root_uri]);
    let specifier = resolve_url("file:///a.ts").unwrap();
    assert!(!config.specifier_enabled(&specifier));
    config.set_workspace_settings(
      serde_json::from_value(json!({
        "enable": true
      }))
      .unwrap(),
      vec![],
    );
    assert!(config.specifier_enabled(&specifier));
  }

  #[test]
  fn test_config_specifier_enabled_path() {
    let root_uri = resolve_url("file:///project/").unwrap();
    let mut config = Config::new_with_roots(vec![root_uri]);
    let specifier_a = resolve_url("file:///project/worker/a.ts").unwrap();
    let specifier_b = resolve_url("file:///project/other/b.ts").unwrap();
    assert!(!config.specifier_enabled(&specifier_a));
    assert!(!config.specifier_enabled(&specifier_b));
    let workspace_settings =
      serde_json::from_str(r#"{ "enablePaths": ["worker"] }"#).unwrap();
    config.set_workspace_settings(workspace_settings, vec![]);
    assert!(config.specifier_enabled(&specifier_a));
    assert!(!config.specifier_enabled(&specifier_b));
  }

  #[test]
  fn test_config_specifier_disabled_path() {
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_roots(vec![root_uri.clone()]);
    config.set_workspace_settings(
      WorkspaceSettings {
        enable: Some(true),
        enable_paths: Some(vec!["mod1.ts".to_string(), "mod2.ts".to_string()]),
        disable_paths: vec!["mod2.ts".to_string()],
        ..Default::default()
      },
      vec![],
    );

    assert!(config.specifier_enabled(&root_uri.join("mod1.ts").unwrap()));
    assert!(!config.specifier_enabled(&root_uri.join("mod2.ts").unwrap()));
    assert!(!config.specifier_enabled(&root_uri.join("mod3.ts").unwrap()));
  }

  #[test]
  fn test_set_workspace_settings_defaults() {
    let mut config = Config::default();
    config.set_workspace_settings(
      serde_json::from_value(json!({})).unwrap(),
      vec![],
    );
    assert_eq!(
      config.workspace_settings().clone(),
      WorkspaceSettings {
        enable: None,
        disable_paths: vec![],
        enable_paths: None,
        cache: None,
        cache_on_save: false,
        certificate_stores: None,
        config: None,
        import_map: None,
        code_lens: CodeLensSettings {
          implementations: false,
          references: false,
          references_all_functions: false,
          test: true,
        },
        internal_debug: false,
        internal_inspect: InspectSetting::Bool(false),
        log_file: false,
        lint: true,
        document_preload_limit: 1_000,
        suggest: DenoCompletionSettings {
          imports: ImportCompletionSettings {
            auto_discover: true,
            hosts: HashMap::new(),
          }
        },
        testing: TestingSettings {
          args: vec!["--allow-all".to_string(), "--no-check".to_string()],
        },
        tls_certificate: None,
        unsafely_ignore_certificate_errors: None,
        unstable: Default::default(),
        javascript: LanguageWorkspaceSettings {
          inlay_hints: InlayHintsSettings {
            parameter_names: InlayHintsParamNamesOptions {
              enabled: InlayHintsParamNamesEnabled::None,
              suppress_when_argument_matches_name: true
            },
            parameter_types: InlayHintsParamTypesOptions { enabled: false },
            variable_types: InlayHintsVarTypesOptions {
              enabled: false,
              suppress_when_type_matches_name: true
            },
            property_declaration_types: InlayHintsPropDeclTypesOptions {
              enabled: false
            },
            function_like_return_types: InlayHintsFuncLikeReturnTypesOptions {
              enabled: false
            },
            enum_member_values: InlayHintsEnumMemberValuesOptions {
              enabled: false
            },
          },
          preferences: LanguagePreferences {
            import_module_specifier: ImportModuleSpecifier::Shortest,
            jsx_attribute_completion_style: JsxAttributeCompletionStyle::Auto,
            auto_import_file_exclude_patterns: vec![],
            use_aliases_for_renames: true,
            quote_style: QuoteStyle::Auto,
            prefer_type_only_auto_imports: false,
          },
          suggest: CompletionSettings {
            complete_function_calls: false,
            include_automatic_optional_chain_completions: true,
            include_completions_for_import_statements: true,
            names: true,
            paths: true,
            auto_imports: true,
            enabled: true,
            class_member_snippets: ClassMemberSnippets { enabled: true },
            object_literal_method_snippets: ObjectLiteralMethodSnippets {
              enabled: true,
            },
          },
          suggestion_actions: SuggestionActionsSettings { enabled: true },
          update_imports_on_file_move: UpdateImportsOnFileMoveOptions {
            enabled: UpdateImportsOnFileMoveEnabled::Prompt
          }
        },
        typescript: LanguageWorkspaceSettings {
          inlay_hints: InlayHintsSettings {
            parameter_names: InlayHintsParamNamesOptions {
              enabled: InlayHintsParamNamesEnabled::None,
              suppress_when_argument_matches_name: true
            },
            parameter_types: InlayHintsParamTypesOptions { enabled: false },
            variable_types: InlayHintsVarTypesOptions {
              enabled: false,
              suppress_when_type_matches_name: true
            },
            property_declaration_types: InlayHintsPropDeclTypesOptions {
              enabled: false
            },
            function_like_return_types: InlayHintsFuncLikeReturnTypesOptions {
              enabled: false
            },
            enum_member_values: InlayHintsEnumMemberValuesOptions {
              enabled: false
            },
          },
          preferences: LanguagePreferences {
            import_module_specifier: ImportModuleSpecifier::Shortest,
            jsx_attribute_completion_style: JsxAttributeCompletionStyle::Auto,
            auto_import_file_exclude_patterns: vec![],
            use_aliases_for_renames: true,
            quote_style: QuoteStyle::Auto,
            prefer_type_only_auto_imports: false,
          },
          suggest: CompletionSettings {
            complete_function_calls: false,
            include_automatic_optional_chain_completions: true,
            include_completions_for_import_statements: true,
            names: true,
            paths: true,
            auto_imports: true,
            enabled: true,
            class_member_snippets: ClassMemberSnippets { enabled: true },
            object_literal_method_snippets: ObjectLiteralMethodSnippets {
              enabled: true,
            },
          },
          suggestion_actions: SuggestionActionsSettings { enabled: true },
          update_imports_on_file_move: UpdateImportsOnFileMoveOptions {
            enabled: UpdateImportsOnFileMoveEnabled::Prompt
          },
        },
        tracing: Default::default()
      }
    );
  }

  #[test]
  fn test_empty_cache() {
    let mut config = Config::default();
    config.set_workspace_settings(
      serde_json::from_value(json!({ "cache": "" })).unwrap(),
      vec![],
    );
    assert_eq!(
      config.workspace_settings().clone(),
      WorkspaceSettings::default()
    );
  }

  #[test]
  fn test_empty_import_map() {
    let mut config = Config::default();
    config.set_workspace_settings(
      serde_json::from_value(json!({ "import_map": "" })).unwrap(),
      vec![],
    );
    assert_eq!(
      config.workspace_settings().clone(),
      WorkspaceSettings::default()
    );
  }

  #[test]
  fn test_empty_tls_certificate() {
    let mut config = Config::default();
    config.set_workspace_settings(
      serde_json::from_value(json!({ "tls_certificate": "" })).unwrap(),
      vec![],
    );
    assert_eq!(
      config.workspace_settings().clone(),
      WorkspaceSettings::default()
    );
  }

  #[test]
  fn test_empty_config() {
    let mut config = Config::default();
    config.set_workspace_settings(
      serde_json::from_value(json!({ "config": "" })).unwrap(),
      vec![],
    );
    assert_eq!(
      config.workspace_settings().clone(),
      WorkspaceSettings::default()
    );
  }

  struct DefaultRegistry;

  #[async_trait::async_trait(?Send)]
  impl deno_lockfile::NpmPackageInfoProvider for DefaultRegistry {
    async fn get_npm_package_info(
      &self,
      values: &[deno_semver::package::PackageNv],
    ) -> Result<
      Vec<deno_lockfile::Lockfile5NpmInfo>,
      Box<dyn std::error::Error + Send + Sync>,
    > {
      Ok(values.iter().map(|_| Default::default()).collect())
    }
  }

  fn default_registry(
  ) -> Arc<dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync> {
    Arc::new(DefaultRegistry)
  }

  #[tokio::test]
  async fn config_enable_via_config_file_detection() {
    let root_uri = root_dir();
    let mut config = Config::new_with_roots(vec![root_uri.clone()]);
    assert!(!config.specifier_enabled(&root_uri));

    config
      .tree
      .inject_config_file(
        ConfigFile::new("{}", root_uri.join("deno.json").unwrap()).unwrap(),
        &default_registry(),
      )
      .await;
    assert!(config.specifier_enabled(&root_uri));
  }

  // Regression test for https://github.com/denoland/vscode_deno/issues/917.
  #[test]
  fn config_specifier_enabled_matches_by_path_component() {
    let root_uri = root_dir();
    let mut config = Config::new_with_roots(vec![root_uri.clone()]);
    config.set_workspace_settings(
      WorkspaceSettings {
        enable_paths: Some(vec!["mo".to_string()]),
        ..Default::default()
      },
      vec![],
    );
    assert!(!config.specifier_enabled(&root_uri.join("mod.ts").unwrap()));
  }

  #[tokio::test]
  async fn config_specifier_enabled_for_test() {
    let root_uri = root_dir();
    let mut config = Config::new_with_roots(vec![root_uri.clone()]);
    let mut settings = WorkspaceSettings {
      enable: Some(true),
      enable_paths: Some(vec!["mod1.ts".to_string(), "mod2.ts".to_string()]),
      disable_paths: vec!["mod2.ts".to_string()],
      ..Default::default()
    };
    config.set_workspace_settings(settings.clone(), vec![]);
    assert!(
      config.specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod3.ts").unwrap())
    );
    settings.enable_paths = None;
    config.set_workspace_settings(settings, vec![]);

    config
      .tree
      .inject_config_file(
        ConfigFile::new(
          &json!({
            "exclude": ["mod2.ts"],
            "test": {
              "exclude": ["mod3.ts"],
            },
          })
          .to_string(),
          root_uri.join("deno.json").unwrap(),
        )
        .unwrap(),
        &default_registry(),
      )
      .await;
    assert!(
      config.specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod3.ts").unwrap())
    );

    config
      .tree
      .inject_config_file(
        ConfigFile::new(
          &json!({
            "test": {
              "include": ["mod1.ts"],
            },
          })
          .to_string(),
          root_uri.join("deno.json").unwrap(),
        )
        .unwrap(),
        &default_registry(),
      )
      .await;

    config
      .tree
      .inject_config_file(
        ConfigFile::new(
          &json!({
            "test": {
              "exclude": ["mod2.ts"],
              "include": ["mod2.ts"],
            },
          })
          .to_string(),
          root_uri.join("deno.json").unwrap(),
        )
        .unwrap(),
        &default_registry(),
      )
      .await;
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap())
    );
  }

  fn root_dir() -> Url {
    if cfg!(windows) {
      Url::parse("file://C:/root/").unwrap()
    } else {
      Url::parse("file:///root/").unwrap()
    }
  }
}
