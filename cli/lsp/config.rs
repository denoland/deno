// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::MediaType;
use deno_config::deno_json::DenoJsonCache;
use deno_config::deno_json::FmtConfig;
use deno_config::deno_json::FmtOptionsConfig;
use deno_config::deno_json::LintConfig;
use deno_config::deno_json::TestConfig;
use deno_config::deno_json::TsConfig;
use deno_config::fs::DenoConfigFs;
use deno_config::fs::RealDenoConfigFs;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPatternSet;
use deno_config::workspace::CreateResolverOptions;
use deno_config::workspace::PackageJsonDepResolution;
use deno_config::workspace::SpecifiedImportMap;
use deno_config::workspace::VendorEnablement;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceCache;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceDirectoryEmptyOptions;
use deno_config::workspace::WorkspaceDiscoverOptions;
use deno_config::workspace::WorkspaceResolver;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde::de::DeserializeOwned;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use deno_lint::linter::LintConfig as DenoLintConfig;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_package_json::PackageJsonCache;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::fs_util::specifier_to_file_path;
use indexmap::IndexSet;
use lsp::Url;
use lsp_types::ClientCapabilities;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::lsp_types as lsp;

use super::logging::lsp_log;
use crate::args::discover_npmrc_from_workspace;
use crate::args::has_flag_env_var;
use crate::args::CliLockfile;
use crate::args::ConfigFile;
use crate::args::LintFlags;
use crate::args::LintOptions;
use crate::args::DENO_FUTURE;
use crate::cache::FastInsecureHasher;
use crate::file_fetcher::FileFetcher;
use crate::lsp::logging::lsp_warn;
use crate::resolver::SloppyImportsResolver;
use crate::tools::lint::CliLinter;
use crate::tools::lint::CliLinterOptions;
use crate::tools::lint::LintRuleProvider;
use crate::util::fs::canonicalize_path_maybe_not_exists;

pub const SETTINGS_SECTION: &str = "deno";

fn is_true() -> bool {
  true
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
}

impl Default for LanguagePreferences {
  fn default() -> Self {
    LanguagePreferences {
      import_module_specifier: Default::default(),
      jsx_attribute_completion_style: Default::default(),
      auto_import_file_exclude_patterns: vec![],
      use_aliases_for_renames: true,
      quote_style: Default::default(),
    }
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
  pub unstable: bool,

  #[serde(default)]
  pub javascript: LanguageWorkspaceSettings,

  #[serde(default)]
  pub typescript: LanguageWorkspaceSettings,
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
      unstable: false,
      javascript: Default::default(),
      typescript: Default::default(),
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
  pub by_workspace_folder:
    BTreeMap<ModuleSpecifier, Option<Arc<WorkspaceSettings>>>,
  pub first_folder: Option<ModuleSpecifier>,
}

impl Settings {
  /// Returns `None` if the value should be deferred to the presence of a
  /// `deno.json` file.
  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> Option<bool> {
    let Ok(path) = specifier_to_file_path(specifier) else {
      // Non-file URLs are not disabled by these settings.
      return Some(true);
    };
    let (settings, mut folder_uri) = self.get_for_specifier(specifier);
    folder_uri = folder_uri.or(self.first_folder.as_ref());
    let mut disable_paths = vec![];
    let mut enable_paths = None;
    if let Some(folder_uri) = folder_uri {
      if let Ok(folder_path) = specifier_to_file_path(folder_uri) {
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
        if path.starts_with(enable_path) {
          return Some(true);
        }
      }
      Some(false)
    } else {
      settings.enable
    }
  }

  pub fn get_unscoped(&self) -> &WorkspaceSettings {
    &self.unscoped
  }

  pub fn get_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> (&WorkspaceSettings, Option<&ModuleSpecifier>) {
    let Ok(path) = specifier_to_file_path(specifier) else {
      return (&self.unscoped, self.first_folder.as_ref());
    };
    for (folder_uri, settings) in self.by_workspace_folder.iter().rev() {
      if let Some(settings) = settings {
        let Ok(folder_path) = specifier_to_file_path(folder_uri) else {
          continue;
        };
        if path.starts_with(folder_path) {
          return (settings, Some(folder_uri));
        }
      }
    }
    (&self.unscoped, self.first_folder.as_ref())
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
  pub workspace_folders: Arc<Vec<(ModuleSpecifier, lsp::WorkspaceFolder)>>,
  pub tree: ConfigTree,
}

impl Config {
  #[cfg(test)]
  pub fn new_with_roots(root_uris: impl IntoIterator<Item = Url>) -> Self {
    let mut config = Self::default();
    let mut folders = vec![];
    for root_uri in root_uris {
      let name = root_uri.path_segments().and_then(|s| s.last());
      let name = name.unwrap_or_default().to_string();
      folders.push((
        root_uri.clone(),
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
    folders: Vec<(ModuleSpecifier, lsp::WorkspaceFolder)>,
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
    folder_settings: Vec<(ModuleSpecifier, WorkspaceSettings)>,
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
      | MediaType::TsBuildInfo
      | MediaType::SourceMap
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

  pub fn root_uri(&self) -> Option<&Url> {
    self.workspace_folders.first().map(|p| &p.0)
  }

  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> bool {
    let data = self.tree.data_for_specifier(specifier);
    if let Some(data) = &data {
      if let Ok(path) = specifier.to_file_path() {
        if data.exclude_files.matches_path(&path) {
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
        "resolveJsonModule": true,
        "strict": true,
        "target": "esnext",
        "useDefineForClassFields": true,
        "useUnknownInCatchVariables": false,
        "jsx": "react",
        "jsxFactory": "React.createElement",
        "jsxFragmentFactory": "React.Fragment",
      })),
    }
  }
}

impl LspTsConfig {
  pub fn new(config_file: Option<&ConfigFile>) -> Self {
    let mut ts_config = Self::default();
    match ts_config.inner.merge_tsconfig_from_config_file(config_file) {
      Ok(Some(ignored_options)) => lsp_warn!("{}", ignored_options),
      Err(err) => lsp_warn!("{}", err),
      _ => {}
    }
    ts_config
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
  pub member_dir: Arc<WorkspaceDirectory>,
  pub fmt_config: Arc<FmtConfig>,
  pub lint_config: Arc<LintConfig>,
  pub test_config: Arc<TestConfig>,
  pub exclude_files: Arc<PathOrPatternSet>,
  pub linter: Arc<CliLinter>,
  pub ts_config: Arc<LspTsConfig>,
  pub byonm: bool,
  pub node_modules_dir: Option<PathBuf>,
  pub vendor_dir: Option<PathBuf>,
  pub lockfile: Option<Arc<CliLockfile>>,
  pub npmrc: Option<Arc<ResolvedNpmRc>>,
  pub resolver: Arc<WorkspaceResolver>,
  pub sloppy_imports_resolver: Option<Arc<SloppyImportsResolver>>,
  pub import_map_from_settings: Option<ModuleSpecifier>,
  watched_files: HashMap<ModuleSpecifier, ConfigWatchedFileType>,
}

impl ConfigData {
  #[allow(clippy::too_many_arguments)]
  async fn load(
    specified_config: Option<&Path>,
    scope: &ModuleSpecifier,
    settings: &Settings,
    file_fetcher: &Arc<FileFetcher>,
    // sync requirement is because the lsp requires sync
    cached_deno_config_fs: &(dyn DenoConfigFs + Sync),
    deno_json_cache: &(dyn DenoJsonCache + Sync),
    pkg_json_cache: &(dyn PackageJsonCache + Sync),
    workspace_cache: &(dyn WorkspaceCache + Sync),
  ) -> Self {
    let scope = Arc::new(scope.clone());
    let discover_result = match scope.to_file_path() {
      Ok(scope_dir_path) => {
        let paths = [scope_dir_path];
        WorkspaceDirectory::discover(
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
            fs: cached_deno_config_fs,
            additional_config_file_names: &[],
            deno_json_cache: Some(deno_json_cache),
            pkg_json_cache: Some(pkg_json_cache),
            workspace_cache: Some(workspace_cache),
            discover_pkg_json: !has_flag_env_var("DENO_NO_PACKAGE_JSON"),
            config_parse_options: Default::default(),
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
        Self::load_inner(member_dir, scope, settings, Some(file_fetcher)).await
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
    file_fetcher: Option<&Arc<FileFetcher>>,
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
    let npmrc = discover_npmrc_from_workspace(&member_dir.workspace)
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
      .map(|(r, _)| r)
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
        .unwrap_or_else(|| {
          LintConfig::new_with_base(default_file_pattern_base.clone())
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

    let ts_config = LspTsConfig::new(
      member_dir.workspace.root_deno_json().map(|c| c.as_ref()),
    );

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
    let lockfile = resolve_lockfile_from_workspace(&member_dir).map(Arc::new);
    if let Some(lockfile) = &lockfile {
      if let Ok(specifier) = ModuleSpecifier::from_file_path(&lockfile.filename)
      {
        add_watched_file(specifier, ConfigWatchedFileType::Lockfile);
      }
    }

    let byonm = std::env::var("DENO_UNSTABLE_BYONM").is_ok()
      || member_dir.workspace.has_unstable("byonm")
      || (*DENO_FUTURE
        && member_dir.workspace.package_jsons().next().is_some()
        && member_dir.workspace.node_modules_dir().is_none());
    if byonm {
      lsp_log!("  Enabled 'bring your own node_modules'.");
    }
    let node_modules_dir =
      resolve_node_modules_dir(&member_dir.workspace, byonm);

    // Mark the import map as a watched file
    if let Some(import_map_specifier) = member_dir
      .workspace
      .to_import_map_specifier()
      .ok()
      .flatten()
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
        // spawn due to the lsp's `Send` requirement
        let fetch_result = deno_core::unsync::spawn({
          let file_fetcher = file_fetcher.cloned().unwrap();
          let import_map_url = import_map_url.clone();
          async move {
            file_fetcher
              .fetch(&import_map_url, &PermissionsContainer::allow_all())
              .await
          }
        })
        .await
        .unwrap();

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
    let resolver = deno_core::unsync::spawn({
      let workspace = member_dir.clone();
      let file_fetcher = file_fetcher.cloned();
      async move {
        workspace
          .create_resolver(
            CreateResolverOptions {
              pkg_json_dep_resolution,
              specified_import_map,
            },
            move |specifier| {
              let specifier = specifier.clone();
              let file_fetcher = file_fetcher.clone().unwrap();
              async move {
                let file = file_fetcher
                  .fetch(&specifier, &PermissionsContainer::allow_all())
                  .await?
                  .into_text_decoded()?;
                Ok(file.source.to_string())
              }
            },
          )
          .await
          .inspect_err(|err| {
            lsp_warn!(
              "  Failed to load resolver: {}",
              err // will contain the specifier
            );
          })
          .ok()
      }
    })
    .await
    .unwrap()
    .unwrap_or_else(|| {
      // create a dummy resolver
      WorkspaceResolver::new_raw(
        scope.clone(),
        None,
        member_dir.workspace.package_jsons().cloned().collect(),
        pkg_json_dep_resolution,
      )
    });
    if !resolver.diagnostics().is_empty() {
      lsp_warn!(
        "  Import map diagnostics:\n{}",
        resolver
          .diagnostics()
          .iter()
          .map(|d| format!("    - {d}"))
          .collect::<Vec<_>>()
          .join("\n")
      );
    }
    let unstable_sloppy_imports = std::env::var("DENO_UNSTABLE_SLOPPY_IMPORTS")
      .is_ok()
      || member_dir.workspace.has_unstable("sloppy-imports");
    let sloppy_imports_resolver = unstable_sloppy_imports.then(|| {
      Arc::new(SloppyImportsResolver::new_without_stat_cache(Arc::new(
        deno_runtime::deno_fs::RealFs,
      )))
    });
    let resolver = Arc::new(resolver);
    let lint_rule_provider = LintRuleProvider::new(
      sloppy_imports_resolver.clone(),
      Some(resolver.clone()),
    );
    let linter = Arc::new(CliLinter::new(CliLinterOptions {
      configured_rules: lint_rule_provider.resolve_lint_rules(
        LintOptions::resolve((*lint_config).clone(), &LintFlags::default())
          .rules,
        member_dir.maybe_deno_json().map(|c| c.as_ref()),
      ),
      fix: false,
      deno_lint_config,
    }));

    ConfigData {
      scope,
      member_dir,
      resolver,
      sloppy_imports_resolver,
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
}

#[derive(Clone, Debug, Default)]
pub struct ConfigTree {
  scopes: Arc<BTreeMap<ModuleSpecifier, Arc<ConfigData>>>,
}

impl ConfigTree {
  pub fn scope_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&ModuleSpecifier> {
    self
      .scopes
      .keys()
      .rfind(|s| specifier.as_str().starts_with(s.as_str()))
  }

  pub fn data_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&Arc<ConfigData>> {
    self
      .scope_for_specifier(specifier)
      .and_then(|s| self.scopes.get(s))
  }

  pub fn data_by_scope(
    &self,
  ) -> &Arc<BTreeMap<ModuleSpecifier, Arc<ConfigData>>> {
    &self.scopes
  }

  pub fn workspace_dir_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
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

  pub fn fmt_config_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Arc<FmtConfig> {
    self
      .data_for_specifier(specifier)
      .map(|d| d.fmt_config.clone())
      .unwrap_or_else(|| Arc::new(FmtConfig::new_with_base(PathBuf::from("/"))))
  }

  /// Returns (scope_uri, type).
  pub fn watched_file_type(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<(&ModuleSpecifier, ConfigWatchedFileType)> {
    for (scope_uri, data) in self.scopes.iter() {
      if let Some(typ) = data.watched_files.get(specifier) {
        return Some((scope_uri, *typ));
      }
    }
    None
  }

  pub fn is_watched_file(&self, specifier: &ModuleSpecifier) -> bool {
    if specifier.path().ends_with("/deno.json")
      || specifier.path().ends_with("/deno.jsonc")
      || specifier.path().ends_with("/package.json")
    {
      return true;
    }
    self
      .scopes
      .values()
      .any(|data| data.watched_files.contains_key(specifier))
  }

  pub async fn refresh(
    &mut self,
    settings: &Settings,
    workspace_files: &IndexSet<ModuleSpecifier>,
    file_fetcher: &Arc<FileFetcher>,
  ) {
    lsp_log!("Refreshing configuration tree...");
    // since we're resolving a workspace multiple times in different
    // folders, we want to cache all the lookups and config files across
    // ConfigData::load calls
    let cached_fs = CachedDenoConfigFs::default();
    let deno_json_cache = DenoJsonMemCache::default();
    let pkg_json_cache = PackageJsonMemCache::default();
    let workspace_cache = WorkspaceMemCache::default();
    let mut scopes = BTreeMap::new();
    for (folder_uri, ws_settings) in &settings.by_workspace_folder {
      let mut ws_settings = ws_settings.as_ref();
      if Some(folder_uri) == settings.first_folder.as_ref() {
        ws_settings = ws_settings.or(Some(&settings.unscoped));
      }
      if let Some(ws_settings) = ws_settings {
        if let Some(config_path) = &ws_settings.config {
          if let Ok(config_uri) = folder_uri.join(config_path) {
            if let Ok(config_file_path) = config_uri.to_file_path() {
              scopes.insert(
                folder_uri.clone(),
                Arc::new(
                  ConfigData::load(
                    Some(&config_file_path),
                    folder_uri,
                    settings,
                    file_fetcher,
                    &cached_fs,
                    &deno_json_cache,
                    &pkg_json_cache,
                    &workspace_cache,
                  )
                  .await,
                ),
              );
            }
          }
        }
      }
    }

    for specifier in workspace_files {
      if !(specifier.path().ends_with("/deno.json")
        || specifier.path().ends_with("/deno.jsonc")
        || specifier.path().ends_with("/package.json"))
      {
        continue;
      }
      let Ok(scope) = specifier.join(".") else {
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
          &cached_fs,
          &deno_json_cache,
          &pkg_json_cache,
          &workspace_cache,
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
          &cached_fs,
          &deno_json_cache,
          &pkg_json_cache,
          &workspace_cache,
        )
        .await;
        scopes.insert(member_scope.as_ref().clone(), Arc::new(member_data));
      }
    }

    for folder_uri in settings.by_workspace_folder.keys() {
      if !scopes
        .keys()
        .any(|s| folder_uri.as_str().starts_with(s.as_str()))
      {
        scopes.insert(
          folder_uri.clone(),
          Arc::new(
            ConfigData::load(
              None,
              folder_uri,
              settings,
              file_fetcher,
              &cached_fs,
              &deno_json_cache,
              &pkg_json_cache,
              &workspace_cache,
            )
            .await,
          ),
        );
      }
    }
    self.scopes = Arc::new(scopes);
  }

  #[cfg(test)]
  pub async fn inject_config_file(&mut self, config_file: ConfigFile) {
    let scope = config_file.specifier.join(".").unwrap();
    let json_text = serde_json::to_string(&config_file.json).unwrap();
    let test_fs = deno_runtime::deno_fs::InMemoryFs::default();
    let config_path = specifier_to_file_path(&config_file.specifier).unwrap();
    test_fs.setup_text_files(vec![(
      config_path.to_string_lossy().to_string(),
      json_text,
    )]);
    let workspace_dir = Arc::new(
      WorkspaceDirectory::discover(
        deno_config::workspace::WorkspaceDiscoverStart::ConfigFile(
          &config_path,
        ),
        &deno_config::workspace::WorkspaceDiscoverOptions {
          fs: &crate::args::deno_json::DenoConfigFsAdapter(&test_fs),
          ..Default::default()
        },
      )
      .unwrap(),
    );
    let data = Arc::new(
      ConfigData::load_inner(
        workspace_dir,
        Arc::new(scope.clone()),
        &Default::default(),
        None,
      )
      .await,
    );
    assert!(data.maybe_deno_json().is_some());
    self.scopes = Arc::new([(scope, data)].into_iter().collect());
  }
}

fn resolve_lockfile_from_workspace(
  workspace: &WorkspaceDirectory,
) -> Option<CliLockfile> {
  let lockfile_path = match workspace.workspace.resolve_lockfile_path() {
    Ok(Some(value)) => value,
    Ok(None) => return None,
    Err(err) => {
      lsp_warn!("Error resolving lockfile: {:#}", err);
      return None;
    }
  };
  resolve_lockfile_from_path(lockfile_path)
}

fn resolve_node_modules_dir(
  workspace: &Workspace,
  byonm: bool,
) -> Option<PathBuf> {
  // For the language server, require an explicit opt-in via the
  // `nodeModulesDir: true` setting in the deno.json file. This is to
  // reduce the chance of modifying someone's node_modules directory
  // without them having asked us to do so.
  let explicitly_disabled = workspace.node_modules_dir() == Some(false);
  if explicitly_disabled {
    return None;
  }
  let enabled = byonm
    || workspace.node_modules_dir() == Some(true)
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

fn resolve_lockfile_from_path(lockfile_path: PathBuf) -> Option<CliLockfile> {
  match CliLockfile::read_from_path(lockfile_path, false) {
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

#[derive(Default)]
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

#[derive(Default)]
struct CachedFsItems<T: Clone> {
  items: HashMap<PathBuf, Result<T, std::io::Error>>,
}

impl<T: Clone> CachedFsItems<T> {
  pub fn get(
    &mut self,
    path: &Path,
    action: impl FnOnce(&Path) -> Result<T, std::io::Error>,
  ) -> Result<T, std::io::Error> {
    let value = if let Some(value) = self.items.get(path) {
      value
    } else {
      let value = action(path);
      // just in case this gets really large for some reason
      if self.items.len() == 16_384 {
        return value;
      }
      self.items.insert(path.to_owned(), value);
      self.items.get(path).unwrap()
    };
    value
      .as_ref()
      .map(|v| (*v).clone())
      .map_err(|e| std::io::Error::new(e.kind(), e.to_string()))
  }
}

#[derive(Default)]
struct InnerData {
  stat_calls: CachedFsItems<deno_config::fs::FsMetadata>,
  read_to_string_calls: CachedFsItems<String>,
}

#[derive(Default)]
struct CachedDenoConfigFs(Mutex<InnerData>);

impl DenoConfigFs for CachedDenoConfigFs {
  fn stat_sync(
    &self,
    path: &Path,
  ) -> Result<deno_config::fs::FsMetadata, std::io::Error> {
    self
      .0
      .lock()
      .stat_calls
      .get(path, |path| RealDenoConfigFs.stat_sync(path))
  }

  fn read_to_string_lossy(
    &self,
    path: &Path,
  ) -> Result<String, std::io::Error> {
    self
      .0
      .lock()
      .read_to_string_calls
      .get(path, |path| RealDenoConfigFs.read_to_string_lossy(path))
  }

  fn read_dir(
    &self,
    path: &Path,
  ) -> Result<Vec<deno_config::fs::FsDirEntry>, std::io::Error> {
    // no need to cache these because the workspace cache will ensure
    // we only do read_dir calls once (read_dirs are only used for
    // npm workspace resolution)
    RealDenoConfigFs.read_dir(path)
  }
}

#[cfg(test)]
mod tests {
  use deno_config::deno_json::ConfigParseOptions;
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
        unstable: false,
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
          update_imports_on_file_move: UpdateImportsOnFileMoveOptions {
            enabled: UpdateImportsOnFileMoveEnabled::Prompt
          }
        },
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

  #[tokio::test]
  async fn config_enable_via_config_file_detection() {
    let root_uri = root_dir();
    let mut config = Config::new_with_roots(vec![root_uri.clone()]);
    assert!(!config.specifier_enabled(&root_uri));

    config
      .tree
      .inject_config_file(
        ConfigFile::new(
          "{}",
          root_uri.join("deno.json").unwrap(),
          &ConfigParseOptions::default(),
        )
        .unwrap(),
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
          &ConfigParseOptions::default(),
        )
        .unwrap(),
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
          &ConfigParseOptions::default(),
        )
        .unwrap(),
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
          &ConfigParseOptions::default(),
        )
        .unwrap(),
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
