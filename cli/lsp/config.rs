// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::documents::file_like_to_file_specifier;
use super::logging::lsp_log;
use crate::args::ConfigFile;
use crate::args::FmtOptions;
use crate::args::LintOptions;
use crate::cache::FastInsecureHasher;
use crate::file_fetcher::FileFetcher;
use crate::lsp::logging::lsp_warn;
use crate::util::fs::canonicalize_path_maybe_not_exists;
use crate::util::path::specifier_to_file_path;
use deno_ast::MediaType;
use deno_config::FmtOptionsConfig;
use deno_config::TsConfig;
use deno_core::parking_lot::Mutex;
use deno_core::serde::de::DeserializeOwned;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use deno_lockfile::Lockfile;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::permissions::PermissionsContainer;
use import_map::ImportMap;
use lsp::Url;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::lsp_types as lsp;

pub const SETTINGS_SECTION: &str = "deno";

#[derive(Debug, Clone, Default)]
pub struct ClientCapabilities {
  pub code_action_disabled_support: bool,
  pub line_folding_only: bool,
  pub snippet_support: bool,
  pub status_notification: bool,
  /// The client provides the `experimental.testingApi` capability, which is
  /// built around VSCode's testing API. It indicates that the server should
  /// send notifications about tests discovered in modules.
  pub testing_api: bool,
  pub workspace_configuration: bool,
  pub workspace_did_change_watched_files: bool,
  pub workspace_will_rename_files: bool,
}

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

#[derive(Debug, Clone, Default)]
pub struct ConfigSnapshot {
  pub client_capabilities: ClientCapabilities,
  pub settings: Settings,
  pub workspace_folders: Vec<(ModuleSpecifier, lsp::WorkspaceFolder)>,
  pub tree: Arc<ConfigTree>,
}

impl ConfigSnapshot {
  pub fn workspace_settings_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> &WorkspaceSettings {
    self.settings.get_for_specifier(specifier).0
  }

  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> bool {
    let config_file = self.tree.config_file_for_specifier(specifier);
    if let Some(cf) = &config_file {
      if let Some(files) = cf.to_files_config().ok().flatten() {
        if !files.matches_specifier(specifier) {
          return false;
        }
      }
    }
    self
      .settings
      .specifier_enabled(specifier)
      .unwrap_or_else(|| config_file.is_some())
  }

  pub fn specifier_enabled_for_test(
    &self,
    specifier: &ModuleSpecifier,
  ) -> bool {
    if let Some(cf) = self.tree.config_file_for_specifier(specifier) {
      if let Some(options) = cf.to_test_config().ok().flatten() {
        if !options.files.matches_specifier(specifier) {
          return false;
        }
      }
    }
    if !self.specifier_enabled(specifier) {
      return false;
    }
    true
  }
}

#[derive(Debug, Default, Clone)]
pub struct Settings {
  pub unscoped: WorkspaceSettings,
  pub by_workspace_folder: BTreeMap<ModuleSpecifier, Option<WorkspaceSettings>>,
}

impl Settings {
  pub fn first_root_uri(&self) -> Option<&ModuleSpecifier> {
    self.by_workspace_folder.first_key_value().map(|e| e.0)
  }

  /// Returns `None` if the value should be deferred to the presence of a
  /// `deno.json` file.
  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> Option<bool> {
    let Ok(path) = specifier_to_file_path(specifier) else {
      // Non-file URLs are not disabled by these settings.
      return Some(true);
    };
    let (settings, mut folder_uri) = self.get_for_specifier(specifier);
    folder_uri = folder_uri.or_else(|| self.first_root_uri());
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
      return (&self.unscoped, None);
    };
    let mut is_first_folder = true;
    for (folder_uri, settings) in self.by_workspace_folder.iter().rev() {
      let mut settings = settings.as_ref();
      if is_first_folder {
        settings = settings.or(Some(&self.unscoped));
      }
      is_first_folder = false;
      if let Some(settings) = settings {
        let Ok(folder_path) = specifier_to_file_path(folder_uri) else {
          continue;
        };
        if path.starts_with(folder_path) {
          return (settings, Some(folder_uri));
        }
      }
    }
    (&self.unscoped, None)
  }

  pub fn set_unscoped(&mut self, mut settings: WorkspaceSettings) {
    // See https://github.com/denoland/vscode_deno/issues/908.
    if settings.enable_paths == Some(vec![]) {
      settings.enable_paths = None;
    }
    self.unscoped = settings;
  }

  pub fn set_for_workspace_folder(
    &mut self,
    folder: &ModuleSpecifier,
    mut settings: WorkspaceSettings,
  ) {
    if let Some(settings_) = self.by_workspace_folder.get_mut(folder) {
      // See https://github.com/denoland/vscode_deno/issues/908.
      if settings.enable_paths == Some(vec![]) {
        settings.enable_paths = None;
      }
      *settings_ = Some(settings);
    }
  }

  pub fn set_workspace_folders(&mut self, folders: Vec<ModuleSpecifier>) {
    self.by_workspace_folder = folders.into_iter().map(|s| (s, None)).collect();
  }

  pub fn workspace_folders_enabled_hash(&self) -> u64 {
    let mut hasher = FastInsecureHasher::default();
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
    hasher.finish()
  }
}

pub fn default_ts_config() -> TsConfig {
  TsConfig::new(json!({
    "allowJs": true,
    "esModuleInterop": true,
    "experimentalDecorators": true,
    "isolatedModules": true,
    "jsx": "react",
    "lib": ["deno.ns", "deno.window"],
    "module": "esnext",
    "moduleDetection": "force",
    "noEmit": true,
    "resolveJsonModule": true,
    "strict": true,
    "target": "esnext",
    "useDefineForClassFields": true,
    "useUnknownInCatchVariables": false,
  }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigWatchedFileType {
  DenoJson,
  Lockfile,
  PackageJson,
  ImportMap,
}

/// Contains the config file and dependent information.
#[derive(Debug, Default, Clone)]
pub struct ConfigData {
  pub config_file: Option<Arc<ConfigFile>>,
  pub fmt_options: Option<Arc<FmtOptions>>,
  pub lint_options: Option<Arc<LintOptions>>,
  pub ts_config: Option<Arc<TsConfig>>,
  pub node_modules_dir: Option<PathBuf>,
  pub lockfile: Option<Arc<Mutex<Lockfile>>>,
  pub package_json: Option<Arc<PackageJson>>,
  pub import_map: Option<Arc<ImportMap>>,
  watched_files: HashMap<ModuleSpecifier, ConfigWatchedFileType>,
}

impl ConfigData {
  async fn load(
    config_file_specifier: Option<&ModuleSpecifier>,
    scope: &ModuleSpecifier,
    settings: &Settings,
    file_fetcher: Option<&FileFetcher>,
  ) -> Self {
    if let Some(specifier) = config_file_specifier {
      match ConfigFile::from_specifier(specifier.clone()) {
        Ok(config_file) => {
          lsp_log!(
            "  Resolved Deno configuration file: \"{}\"",
            config_file.specifier.as_str()
          );
          Self::load_inner(Some(config_file), scope, settings, file_fetcher)
            .await
        }
        Err(err) => {
          lsp_warn!(
            "  Couldn't read Deno configuration file \"{}\": {}",
            specifier.as_str(),
            err
          );
          let mut data =
            Self::load_inner(None, scope, settings, file_fetcher).await;
          data
            .watched_files
            .insert(specifier.clone(), ConfigWatchedFileType::DenoJson);
          let canonicalized_specifier = specifier
            .to_file_path()
            .ok()
            .and_then(|p| canonicalize_path_maybe_not_exists(&p).ok())
            .and_then(|p| ModuleSpecifier::from_file_path(p).ok());
          if let Some(specifier) = canonicalized_specifier {
            data
              .watched_files
              .insert(specifier, ConfigWatchedFileType::DenoJson);
          }
          data
        }
      }
    } else {
      Self::load_inner(None, scope, settings, file_fetcher).await
    }
  }

  async fn load_inner(
    config_file: Option<ConfigFile>,
    scope: &ModuleSpecifier,
    settings: &Settings,
    file_fetcher: Option<&FileFetcher>,
  ) -> Self {
    let (settings, workspace_folder) = settings.get_for_specifier(scope);
    let mut watched_files = HashMap::with_capacity(6);
    if let Some(config_file) = &config_file {
      watched_files
        .entry(config_file.specifier.clone())
        .or_insert(ConfigWatchedFileType::DenoJson);
    }
    let config_file_canonicalized_specifier = config_file
      .as_ref()
      .and_then(|c| c.specifier.to_file_path().ok())
      .and_then(|p| canonicalize_path_maybe_not_exists(&p).ok())
      .and_then(|p| ModuleSpecifier::from_file_path(p).ok());
    if let Some(specifier) = config_file_canonicalized_specifier {
      watched_files
        .entry(specifier)
        .or_insert(ConfigWatchedFileType::DenoJson);
    }

    // Resolve some config file fields ahead of time
    let fmt_options = config_file.as_ref().map(|config_file| match config_file
      .to_fmt_config()
      .and_then(|o| FmtOptions::resolve(o, None))
    {
      Ok(fmt_options) => fmt_options,
      Err(err) => {
        lsp_warn!("  Couldn't read formatter configuration: {}", err);
        Default::default()
      }
    });
    let lint_options =
      config_file.as_ref().map(|config_file| {
        match config_file
          .to_lint_config()
          .and_then(|o| LintOptions::resolve(o, None))
        {
          Ok(lint_options) => lint_options,
          Err(err) => {
            lsp_warn!("  Couldn't read lint configuration: {}", err);
            Default::default()
          }
        }
      });
    let mut ts_config = default_ts_config();
    if settings.unstable {
      let unstable_libs = json!({
        "lib": ["deno.ns", "deno.window", "deno.unstable"]
      });
      ts_config.merge(&unstable_libs);
    }
    if let Some(config_file) = &config_file {
      match config_file.to_compiler_options() {
        Ok((value, maybe_ignored_options)) => {
          ts_config.merge(&value);
          if let Some(ignored_options) = maybe_ignored_options {
            lsp_warn!("{}", ignored_options);
          }
        }
        Err(err) => lsp_warn!("{}", err),
      }
    }
    let node_modules_dir =
      config_file.as_ref().and_then(resolve_node_modules_dir);

    // Load lockfile
    let lockfile = config_file.as_ref().and_then(resolve_lockfile_from_config);
    if let Some(lockfile) = &lockfile {
      if let Ok(specifier) = ModuleSpecifier::from_file_path(&lockfile.filename)
      {
        watched_files
          .entry(specifier)
          .or_insert(ConfigWatchedFileType::Lockfile);
      }
    }
    let lockfile_canonicalized_specifier = lockfile
      .as_ref()
      .and_then(|lockfile| {
        canonicalize_path_maybe_not_exists(&lockfile.filename).ok()
      })
      .and_then(|p| ModuleSpecifier::from_file_path(p).ok());
    if let Some(specifier) = lockfile_canonicalized_specifier {
      watched_files
        .entry(specifier)
        .or_insert(ConfigWatchedFileType::Lockfile);
    }

    // Load package.json
    let mut package_json = None;
    if let Ok(path) = specifier_to_file_path(scope) {
      let path = path.join("package.json");
      if let Ok(specifier) = ModuleSpecifier::from_file_path(&path) {
        watched_files
          .entry(specifier)
          .or_insert(ConfigWatchedFileType::PackageJson);
      }
      let package_json_canonicalized_specifier =
        canonicalize_path_maybe_not_exists(&path)
          .ok()
          .and_then(|p| ModuleSpecifier::from_file_path(p).ok());
      if let Some(specifier) = package_json_canonicalized_specifier {
        watched_files
          .entry(specifier)
          .or_insert(ConfigWatchedFileType::PackageJson);
      }
      if let Ok(source) = std::fs::read_to_string(&path) {
        match PackageJson::load_from_string(path.clone(), source) {
          Ok(result) => {
            lsp_log!("  Resolved package.json: \"{}\"", path.display());
            package_json = Some(result);
          }
          Err(err) => {
            lsp_warn!(
              "  Couldn't read package.json \"{}\": {}",
              path.display(),
              err
            );
          }
        }
      }
    }

    // Load import map
    let mut import_map = None;
    let mut import_map_value = None;
    let mut import_map_specifier = None;
    if let Some(config_file) = &config_file {
      if config_file.is_an_import_map() {
        import_map_value = Some(config_file.to_import_map_value());
        import_map_specifier = Some(config_file.specifier.clone());
      } else if let Some(import_map_str) = config_file.to_import_map_path() {
        if let Ok(specifier) = config_file.specifier.join(&import_map_str) {
          import_map_specifier = Some(specifier);
        }
      }
    } else if let Some(import_map_str) = &settings.import_map {
      if let Ok(specifier) = Url::parse(import_map_str) {
        import_map_specifier = Some(specifier);
      } else if let Some(folder_uri) = workspace_folder {
        if let Ok(specifier) = folder_uri.join(import_map_str) {
          import_map_specifier = Some(specifier);
        }
      }
    }
    if let Some(specifier) = &import_map_specifier {
      if let Ok(path) = specifier_to_file_path(specifier) {
        watched_files
          .entry(specifier.clone())
          .or_insert(ConfigWatchedFileType::ImportMap);
        let import_map_canonicalized_specifier =
          canonicalize_path_maybe_not_exists(&path)
            .ok()
            .and_then(|p| ModuleSpecifier::from_file_path(p).ok());
        if let Some(specifier) = import_map_canonicalized_specifier {
          watched_files
            .entry(specifier)
            .or_insert(ConfigWatchedFileType::ImportMap);
        }
      }
      if import_map_value.is_none() {
        if let Some(file_fetcher) = file_fetcher {
          let fetch_result = file_fetcher
            .fetch(specifier, PermissionsContainer::allow_all())
            .await;
          let value_result = fetch_result.and_then(|f| {
            serde_json::from_str::<Value>(&f.source).map_err(|e| e.into())
          });
          match value_result {
            Ok(value) => {
              import_map_value = Some(value);
            }
            Err(err) => {
              lsp_warn!(
                "  Couldn't read import map \"{}\": {}",
                specifier.as_str(),
                err
              );
            }
          }
        }
      }
    }
    if let (Some(value), Some(specifier)) =
      (import_map_value, import_map_specifier)
    {
      match import_map::parse_from_value(&specifier, value) {
        Ok(result) => {
          lsp_log!("  Resolved import map: \"{}\"", specifier.as_str());
          if !result.diagnostics.is_empty() {
            lsp_warn!(
              "  Import map diagnostics:\n{}",
              result
                .diagnostics
                .iter()
                .map(|d| format!("    - {d}"))
                .collect::<Vec<_>>()
                .join("\n")
            );
          }
          import_map = Some(result.import_map);
        }
        Err(err) => {
          lsp_warn!(
            "Couldn't read import map \"{}\": {}",
            specifier.as_str(),
            err
          );
        }
      }
    }

    ConfigData {
      config_file: config_file.map(Arc::new),
      fmt_options: fmt_options.map(Arc::new),
      lint_options: lint_options.map(Arc::new),
      ts_config: Some(Arc::new(ts_config)),
      node_modules_dir,
      lockfile: lockfile.map(Mutex::new).map(Arc::new),
      package_json: package_json.map(Arc::new),
      import_map: import_map.map(Arc::new),
      watched_files,
    }
  }
}

#[derive(Debug, Default)]
pub struct ConfigTree {
  scopes: Mutex<BTreeMap<ModuleSpecifier, Arc<ConfigData>>>,
}

impl ConfigTree {
  pub fn data_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Arc<ConfigData>> {
    let specifier = file_like_to_file_specifier(specifier);
    self
      .scopes
      .lock()
      .iter()
      .rfind(|(s, _)| specifier.as_str().starts_with(s.as_str()))
      .map(|(_, d)| d.clone())
  }

  pub fn data_by_scope(&self) -> BTreeMap<ModuleSpecifier, Arc<ConfigData>> {
    self.scopes.lock().clone()
  }

  pub fn scope_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let specifier = file_like_to_file_specifier(specifier);
    self
      .scopes
      .lock()
      .keys()
      .rfind(|s| specifier.as_str().starts_with(s.as_str()))
      .cloned()
  }

  pub fn config_file_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Arc<ConfigFile>> {
    let specifier = file_like_to_file_specifier(specifier);
    self
      .scopes
      .lock()
      .iter()
      .rfind(|(s, _)| specifier.as_str().starts_with(s.as_str()))
      .and_then(|(_, d)| d.config_file.clone())
  }

  pub fn has_config_file_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> bool {
    let specifier = file_like_to_file_specifier(specifier);
    self
      .scopes
      .lock()
      .iter()
      .rfind(|(s, _)| specifier.as_str().starts_with(s.as_str()))
      .map(|(_, d)| d.config_file.is_some())
      .unwrap_or(false)
  }

  pub fn config_files(&self) -> Vec<Arc<ConfigFile>> {
    self
      .scopes
      .lock()
      .iter()
      .filter_map(|(_, d)| d.config_file.clone())
      .collect()
  }

  pub fn package_jsons(&self) -> Vec<Arc<PackageJson>> {
    self
      .scopes
      .lock()
      .iter()
      .filter_map(|(_, d)| d.package_json.clone())
      .collect()
  }

  pub fn fmt_options_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Arc<FmtOptions> {
    let specifier = file_like_to_file_specifier(specifier);
    self
      .scopes
      .lock()
      .iter()
      .rfind(|(s, _)| specifier.as_str().starts_with(s.as_str()))
      .and_then(|(_, d)| d.fmt_options.clone())
      .unwrap_or_default()
  }

  pub fn lint_options_by_scope(
    &self,
  ) -> BTreeMap<ModuleSpecifier, Arc<LintOptions>> {
    self
      .scopes
      .lock()
      .iter()
      .map(|(scope, data)| {
        (scope.clone(), data.lint_options.clone().unwrap_or_default())
      })
      .collect()
  }

  pub fn ts_config_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Arc<TsConfig> {
    let specifier = file_like_to_file_specifier(specifier);
    self
      .scopes
      .lock()
      .iter()
      .rfind(|(s, _)| specifier.as_str().starts_with(s.as_str()))
      .and_then(|(_, d)| d.ts_config.clone())
      .unwrap_or_else(|| Arc::new(default_ts_config()))
  }

  pub fn vendor_dirs_by_scope(&self) -> BTreeMap<ModuleSpecifier, PathBuf> {
    self
      .scopes
      .lock()
      .iter()
      .filter_map(|(scope, data)| {
        Some((scope.clone(), data.config_file.as_ref()?.vendor_dir_path()?))
      })
      .collect()
  }

  pub fn import_map_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Arc<ImportMap>> {
    let specifier = file_like_to_file_specifier(specifier);
    self
      .scopes
      .lock()
      .iter()
      .rfind(|(s, _)| specifier.as_str().starts_with(s.as_str()))
      .and_then(|(_, d)| d.import_map.clone())
  }

  pub async fn refresh(
    &self,
    settings: &Settings,
    workspace_files: &BTreeSet<ModuleSpecifier>,
    file_fetcher: &FileFetcher,
  ) {
    lsp_log!("Refreshing configuration tree...");
    let mut scopes = BTreeMap::new();

    let mut is_first_folder = true;
    for (folder_uri, ws_settings) in &settings.by_workspace_folder {
      let mut ws_settings = ws_settings.as_ref();
      if is_first_folder {
        ws_settings = ws_settings.or(Some(&settings.unscoped));
      }
      is_first_folder = false;
      if let Some(ws_settings) = ws_settings {
        if let Some(config_path) = &ws_settings.config {
          if let Ok(config_uri) = folder_uri.join(config_path) {
            scopes.insert(
              folder_uri.clone(),
              Arc::new(
                ConfigData::load(
                  Some(&config_uri),
                  folder_uri,
                  settings,
                  Some(file_fetcher),
                )
                .await,
              ),
            );
          }
        }
      }
    }

    for specifier in workspace_files {
      if specifier.path().ends_with("/deno.json")
        || specifier.path().ends_with("/deno.jsonc")
      {
        if let Ok(scope) = specifier.join(".") {
          let entry = scopes.entry(scope.clone());
          #[allow(clippy::map_entry)]
          if matches!(entry, std::collections::btree_map::Entry::Vacant(_)) {
            let data = ConfigData::load(
              Some(specifier),
              &scope,
              settings,
              Some(file_fetcher),
            )
            .await;
            entry.or_insert(Arc::new(data));
          }
        }
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
            ConfigData::load(None, folder_uri, settings, Some(file_fetcher))
              .await,
          ),
        );
      }
    }

    *self.scopes.lock() = scopes;
  }

  pub fn watched_file_type(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ConfigWatchedFileType> {
    for data in self.scopes.lock().values() {
      if let Some(typ) = data.watched_files.get(specifier) {
        return Some(*typ);
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
      .lock()
      .values()
      .any(|data| data.watched_files.contains_key(specifier))
  }

  #[cfg(test)]
  pub async fn inject_config_file(&self, config_file: ConfigFile) {
    let scope = config_file.specifier.join(".").unwrap();
    let data = ConfigData::load_inner(
      Some(config_file),
      &scope,
      &Default::default(),
      None,
    )
    .await;
    self.scopes.lock().insert(scope, Arc::new(data));
  }
}

#[derive(Debug)]
pub struct Config {
  pub client_capabilities: ClientCapabilities,
  pub settings: Settings,
  pub workspace_folders: Vec<(ModuleSpecifier, lsp::WorkspaceFolder)>,
  pub tree: Arc<ConfigTree>,
}

impl Config {
  pub fn new() -> Self {
    Self {
      client_capabilities: ClientCapabilities::default(),
      // Root provided by the initialization parameters.
      settings: Default::default(),
      workspace_folders: vec![],
      tree: Default::default(),
    }
  }

  #[cfg(test)]
  pub fn new_with_root(root_uri: Url) -> Self {
    let mut config = Self::new();
    let name = root_uri.path_segments().and_then(|s| s.last());
    let name = name.unwrap_or_default().to_string();
    config.set_workspace_folders(vec![(
      root_uri.clone(),
      lsp::WorkspaceFolder {
        uri: root_uri,
        name,
      },
    )]);
    config
  }

  pub fn set_workspace_folders(
    &mut self,
    folders: Vec<(ModuleSpecifier, lsp::WorkspaceFolder)>,
  ) {
    self
      .settings
      .set_workspace_folders(folders.iter().map(|p| p.0.clone()).collect());
    self.workspace_folders = folders;
  }

  pub fn set_workspace_settings(
    &mut self,
    unscoped: WorkspaceSettings,
    folder_settings: Vec<(ModuleSpecifier, WorkspaceSettings)>,
  ) {
    self.settings.set_unscoped(unscoped);
    for (folder_uri, settings) in folder_settings.into_iter() {
      self
        .settings
        .set_for_workspace_folder(&folder_uri, settings);
    }
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
    if specifier.scheme() == "deno-notebook-cell" {
      return Some(&workspace_settings.typescript);
    }
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

  /// Determine if any code lenses are enabled at all.  This allows short
  /// circuiting when there are no code lenses enabled.
  pub fn enabled_code_lens_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> bool {
    let settings = self.workspace_settings_for_specifier(specifier);
    settings.code_lens.implementations
      || settings.code_lens.references
      || settings.code_lens.test
  }

  pub fn enabled_code_lens_test_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> bool {
    let settings = self.workspace_settings_for_specifier(specifier);
    settings.code_lens.test
  }

  pub fn root_uri(&self) -> Option<&Url> {
    self.workspace_folders.get(0).map(|p| &p.0)
  }

  pub fn snapshot(&self) -> Arc<ConfigSnapshot> {
    Arc::new(ConfigSnapshot {
      client_capabilities: self.client_capabilities.clone(),
      settings: self.settings.clone(),
      workspace_folders: self.workspace_folders.clone(),
      tree: self.tree.clone(),
    })
  }

  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> bool {
    let config_file = self.tree.config_file_for_specifier(specifier);
    if let Some(cf) = &config_file {
      if let Some(files) = cf.to_files_config().ok().flatten() {
        if !files.matches_specifier(specifier) {
          return false;
        }
      }
    }
    self
      .settings
      .specifier_enabled(specifier)
      .unwrap_or_else(|| config_file.is_some())
  }

  pub fn specifier_enabled_for_test(
    &self,
    specifier: &ModuleSpecifier,
  ) -> bool {
    if let Some(cf) = self.tree.config_file_for_specifier(specifier) {
      if let Some(options) = cf.to_test_config().ok().flatten() {
        if !options.files.matches_specifier(specifier) {
          return false;
        }
      }
    }
    if !self.specifier_enabled(specifier) {
      return false;
    }
    true
  }

  pub fn update_capabilities(
    &mut self,
    capabilities: &lsp::ClientCapabilities,
  ) {
    if let Some(experimental) = &capabilities.experimental {
      self.client_capabilities.status_notification = experimental
        .get("statusNotification")
        .and_then(|it| it.as_bool())
        == Some(true);
      self.client_capabilities.testing_api =
        experimental.get("testingApi").and_then(|it| it.as_bool())
          == Some(true);
    }

    if let Some(workspace) = &capabilities.workspace {
      self.client_capabilities.workspace_configuration =
        workspace.configuration.unwrap_or(false);
      self.client_capabilities.workspace_did_change_watched_files = workspace
        .did_change_watched_files
        .and_then(|it| it.dynamic_registration)
        .unwrap_or(false);
      if let Some(file_operations) = &workspace.file_operations {
        if let Some(true) = file_operations.dynamic_registration {
          self.client_capabilities.workspace_will_rename_files =
            file_operations.will_rename.unwrap_or(false);
        }
      }
    }

    if let Some(text_document) = &capabilities.text_document {
      self.client_capabilities.line_folding_only = text_document
        .folding_range
        .as_ref()
        .and_then(|it| it.line_folding_only)
        .unwrap_or(false);
      self.client_capabilities.code_action_disabled_support = text_document
        .code_action
        .as_ref()
        .and_then(|it| it.disabled_support)
        .unwrap_or(false);
      self.client_capabilities.snippet_support =
        if let Some(completion) = &text_document.completion {
          completion
            .completion_item
            .as_ref()
            .and_then(|it| it.snippet_support)
            .unwrap_or(false)
        } else {
          false
        };
    }
  }
}

fn resolve_lockfile_from_config(config_file: &ConfigFile) -> Option<Lockfile> {
  let lockfile_path = match config_file.resolve_lockfile_path() {
    Ok(Some(value)) => value,
    Ok(None) => return None,
    Err(err) => {
      lsp_warn!("Error resolving lockfile: {:#}", err);
      return None;
    }
  };
  resolve_lockfile_from_path(lockfile_path)
}

fn resolve_node_modules_dir(config_file: &ConfigFile) -> Option<PathBuf> {
  // For the language server, require an explicit opt-in via the
  // `nodeModulesDir: true` setting in the deno.json file. This is to
  // reduce the chance of modifying someone's node_modules directory
  // without them having asked us to do so.
  let explicitly_disabled = config_file.node_modules_dir_flag() == Some(false);
  if explicitly_disabled {
    return None;
  }
  let enabled = config_file.node_modules_dir_flag() == Some(true)
    || config_file.vendor_dir_flag() == Some(true);
  if !enabled {
    return None;
  }
  if config_file.specifier.scheme() != "file" {
    return None;
  }
  let file_path = config_file.specifier.to_file_path().ok()?;
  let node_modules_dir = file_path.parent()?.join("node_modules");
  canonicalize_path_maybe_not_exists(&node_modules_dir).ok()
}

fn resolve_lockfile_from_path(lockfile_path: PathBuf) -> Option<Lockfile> {
  match Lockfile::new(lockfile_path, false) {
    Ok(value) => {
      if let Ok(specifier) = ModuleSpecifier::from_file_path(&value.filename) {
        lsp_log!("  Resolved lockfile: \"{}\"", specifier);
      }
      Some(value)
    }
    Err(err) => {
      lsp_warn!("Error loading lockfile: {:#}", err);
      None
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::resolve_url;
  use deno_core::serde_json;
  use deno_core::serde_json::json;
  use pretty_assertions::assert_eq;

  #[test]
  fn test_config_specifier_enabled() {
    let root_uri = resolve_url("file:///").unwrap();
    let mut config = Config::new_with_root(root_uri);
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
    let mut config = Config::new_with_root(root_uri);
    let specifier = resolve_url("file:///a.ts").unwrap();
    assert!(!config.specifier_enabled(&specifier));
    config.set_workspace_settings(
      serde_json::from_value(json!({
        "enable": true
      }))
      .unwrap(),
      vec![],
    );
    let config_snapshot = config.snapshot();
    assert!(config_snapshot.specifier_enabled(&specifier));
  }

  #[test]
  fn test_config_specifier_enabled_path() {
    let root_uri = resolve_url("file:///project/").unwrap();
    let mut config = Config::new_with_root(root_uri);
    let specifier_a = resolve_url("file:///project/worker/a.ts").unwrap();
    let specifier_b = resolve_url("file:///project/other/b.ts").unwrap();
    assert!(!config.specifier_enabled(&specifier_a));
    assert!(!config.specifier_enabled(&specifier_b));
    let workspace_settings =
      serde_json::from_str(r#"{ "enablePaths": ["worker"] }"#).unwrap();
    config.set_workspace_settings(workspace_settings, vec![]);
    dbg!(&config.settings);
    assert!(config.specifier_enabled(&specifier_a));
    assert!(!config.specifier_enabled(&specifier_b));
    let config_snapshot = config.snapshot();
    assert!(config_snapshot.specifier_enabled(&specifier_a));
    assert!(!config_snapshot.specifier_enabled(&specifier_b));
  }

  #[test]
  fn test_config_specifier_disabled_path() {
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_root(root_uri.clone());
    config.settings.unscoped.enable = Some(true);
    config.settings.unscoped.enable_paths =
      Some(vec!["mod1.ts".to_string(), "mod2.ts".to_string()]);
    config.settings.unscoped.disable_paths = vec!["mod2.ts".to_string()];

    assert!(config.specifier_enabled(&root_uri.join("mod1.ts").unwrap()));
    assert!(!config.specifier_enabled(&root_uri.join("mod2.ts").unwrap()));
    assert!(!config.specifier_enabled(&root_uri.join("mod3.ts").unwrap()));
  }

  #[test]
  fn test_set_workspace_settings_defaults() {
    let mut config = Config::new();
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
    let mut config = Config::new();
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
    let mut config = Config::new();
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
    let mut config = Config::new();
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
    let mut config = Config::new();
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
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_root(root_uri.clone());
    config.settings.unscoped.enable = None;
    assert!(!config.specifier_enabled(&root_uri));

    config
      .tree
      .inject_config_file(
        ConfigFile::new("{}", root_uri.join("deno.json").unwrap()).unwrap(),
      )
      .await;
    assert!(config.specifier_enabled(&root_uri));
  }

  // Regression test for https://github.com/denoland/vscode_deno/issues/917.
  #[test]
  fn config_specifier_enabled_matches_by_path_component() {
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_root(root_uri.clone());
    config.settings.unscoped.enable_paths = Some(vec!["mo".to_string()]);
    assert!(!config.specifier_enabled(&root_uri.join("mod.ts").unwrap()));
  }

  #[tokio::test]
  async fn config_specifier_enabled_for_test() {
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_root(root_uri.clone());
    config.settings.unscoped.enable = Some(true);

    config.settings.unscoped.enable_paths =
      Some(vec!["mod1.ts".to_string(), "mod2.ts".to_string()]);
    config.settings.unscoped.disable_paths = vec!["mod2.ts".to_string()];
    assert!(
      config.specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod3.ts").unwrap())
    );
    config.settings.unscoped.enable_paths = None;

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
      )
      .await;
    assert!(
      config.specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap())
    );

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
      )
      .await;
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap())
    );
  }

  #[tokio::test]
  async fn config_snapshot_specifier_enabled_for_test() {
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_root(root_uri.clone());
    config.settings.unscoped.enable = Some(true);
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
      )
      .await;
    let config_snapshot = config.snapshot();
    assert!(config_snapshot
      .specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap()));
    assert!(!config_snapshot
      .specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap()));
    assert!(!config_snapshot
      .specifier_enabled_for_test(&root_uri.join("mod3.ts").unwrap()));
  }
}
