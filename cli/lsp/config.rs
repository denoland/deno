// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::logging::lsp_log;
use crate::args::ConfigFile;
use crate::lsp::logging::lsp_warn;
use crate::util::fs::canonicalize_path_maybe_not_exists;
use crate::util::path::specifier_to_file_path;
use deno_ast::MediaType;
use deno_core::parking_lot::Mutex;
use deno_core::serde::de::DeserializeOwned;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use deno_lockfile::Lockfile;
use lsp::Url;
use std::collections::BTreeMap;
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

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensSpecifierSettings {
  /// Flag for providing test code lens on `Deno.test` statements.  There is
  /// also the `test_args` setting, but this is not used by the server.
  #[serde(default = "is_true")]
  pub test: bool,
}

impl Default for CodeLensSpecifierSettings {
  fn default() -> Self {
    Self { test: true }
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

/// Deno language server specific settings that can be applied uniquely to a
/// specifier.
#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpecifierSettings {
  /// A flag that indicates if Deno is enabled for this specifier or not.
  pub enable: Option<bool>,
  /// A list of paths, using the workspace folder as a base that should be Deno
  /// disabled.
  #[serde(default)]
  pub disable_paths: Vec<String>,
  /// A list of paths, using the workspace folder as a base that should be Deno
  /// enabled.
  pub enable_paths: Option<Vec<String>>,
  /// Code lens specific settings for the resource.
  #[serde(default)]
  pub code_lens: CodeLensSpecifierSettings,
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
}

impl Default for LanguagePreferences {
  fn default() -> Self {
    LanguagePreferences {
      import_module_specifier: Default::default(),
      jsx_attribute_completion_style: Default::default(),
      auto_import_file_exclude_patterns: vec![],
      use_aliases_for_renames: true,
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

  /// Determine if any code lenses are enabled at all.  This allows short
  /// circuiting when there are no code lenses enabled.
  pub fn enabled_code_lens(&self) -> bool {
    self.code_lens.implementations || self.code_lens.references
  }

  pub fn language_settings_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&LanguageWorkspaceSettings> {
    match MediaType::from_specifier(specifier) {
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::Mjs
      | MediaType::Cjs => Some(&self.javascript),
      MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Tsx => Some(&self.typescript),
      MediaType::Json
      | MediaType::Wasm
      | MediaType::TsBuildInfo
      | MediaType::SourceMap
      | MediaType::Unknown => None,
    }
  }

  /// Determine if any inlay hints are enabled. This allows short circuiting
  /// when there are no inlay hints enabled.
  pub fn enabled_inlay_hints(&self, specifier: &ModuleSpecifier) -> bool {
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
}

#[derive(Debug, Clone, Default)]
pub struct ConfigSnapshot {
  pub client_capabilities: ClientCapabilities,
  pub config_file: Option<ConfigFile>,
  pub settings: Settings,
  pub workspace_folders: Vec<(ModuleSpecifier, lsp::WorkspaceFolder)>,
}

impl ConfigSnapshot {
  /// Determine if the provided specifier is enabled or not.
  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> bool {
    specifier_enabled(
      specifier,
      self.config_file.as_ref(),
      &self.settings,
      &self.workspace_folders,
    )
  }

  pub fn specifier_enabled_for_test(
    &self,
    specifier: &ModuleSpecifier,
  ) -> bool {
    if let Some(cf) = &self.config_file {
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
  pub specifiers: BTreeMap<ModuleSpecifier, SpecifierSettings>,
  pub workspace: WorkspaceSettings,
}

#[derive(Debug)]
struct WithCanonicalizedSpecifier<T> {
  /// Stored canonicalized specifier, which is used for file watcher events.
  canonicalized_specifier: ModuleSpecifier,
  file: T,
}

/// Contains the config file and dependent information.
#[derive(Debug)]
struct LspConfigFileInfo {
  config_file: WithCanonicalizedSpecifier<ConfigFile>,
  /// An optional deno.lock file, which is resolved relative to the config file.
  maybe_lockfile: Option<WithCanonicalizedSpecifier<Arc<Mutex<Lockfile>>>>,
  /// The canonicalized node_modules directory, which is found relative to the config file.
  maybe_node_modules_dir: Option<PathBuf>,
}

#[derive(Debug)]
pub struct Config {
  pub client_capabilities: ClientCapabilities,
  settings: Settings,
  pub workspace_folders: Vec<(ModuleSpecifier, lsp::WorkspaceFolder)>,
  /// An optional configuration file which has been specified in the client
  /// options along with some data that is computed after the config file is set.
  maybe_config_file_info: Option<LspConfigFileInfo>,
}

impl Config {
  pub fn new() -> Self {
    Self {
      client_capabilities: ClientCapabilities::default(),
      /// Root provided by the initialization parameters.
      settings: Default::default(),
      workspace_folders: vec![],
      maybe_config_file_info: None,
    }
  }

  #[cfg(test)]
  pub fn new_with_root(root_uri: Url) -> Self {
    let mut config = Self::new();
    let name = root_uri.path_segments().and_then(|s| s.last());
    let name = name.unwrap_or_default().to_string();
    config.workspace_folders = vec![(
      root_uri.clone(),
      lsp::WorkspaceFolder {
        uri: root_uri,
        name,
      },
    )];
    config
  }

  pub fn root_uri(&self) -> Option<&Url> {
    self.workspace_folders.get(0).map(|p| &p.0)
  }

  pub fn maybe_node_modules_dir_path(&self) -> Option<&PathBuf> {
    self
      .maybe_config_file_info
      .as_ref()
      .and_then(|p| p.maybe_node_modules_dir.as_ref())
  }

  pub fn maybe_vendor_dir_path(&self) -> Option<PathBuf> {
    self.maybe_config_file().and_then(|c| c.vendor_dir_path())
  }

  pub fn maybe_config_file(&self) -> Option<&ConfigFile> {
    self
      .maybe_config_file_info
      .as_ref()
      .map(|c| &c.config_file.file)
  }

  /// Canonicalized specifier of the config file, which should only be used for
  /// file watcher events. Otherwise, prefer using the non-canonicalized path
  /// as the rest of the CLI does for config files.
  pub fn maybe_config_file_canonicalized_specifier(
    &self,
  ) -> Option<&ModuleSpecifier> {
    self
      .maybe_config_file_info
      .as_ref()
      .map(|c| &c.config_file.canonicalized_specifier)
  }

  pub fn maybe_lockfile(&self) -> Option<&Arc<Mutex<Lockfile>>> {
    self
      .maybe_config_file_info
      .as_ref()
      .and_then(|c| c.maybe_lockfile.as_ref().map(|l| &l.file))
  }

  /// Canonicalized specifier of the lockfile, which should only be used for
  /// file watcher events. Otherwise, prefer using the non-canonicalized path
  /// as the rest of the CLI does for config files.
  pub fn maybe_lockfile_canonicalized_specifier(
    &self,
  ) -> Option<&ModuleSpecifier> {
    self.maybe_config_file_info.as_ref().and_then(|c| {
      c.maybe_lockfile
        .as_ref()
        .map(|l| &l.canonicalized_specifier)
    })
  }

  pub fn clear_config_file(&mut self) {
    self.maybe_config_file_info = None;
  }

  pub fn has_config_file(&self) -> bool {
    self.maybe_config_file_info.is_some()
  }

  pub fn set_config_file(&mut self, config_file: ConfigFile) {
    self.maybe_config_file_info = Some(LspConfigFileInfo {
      maybe_lockfile: resolve_lockfile_from_config(&config_file).map(
        |lockfile| {
          let path = canonicalize_path_maybe_not_exists(&lockfile.filename)
            .unwrap_or_else(|_| lockfile.filename.clone());
          WithCanonicalizedSpecifier {
            canonicalized_specifier: ModuleSpecifier::from_file_path(path)
              .unwrap(),
            file: Arc::new(Mutex::new(lockfile)),
          }
        },
      ),
      maybe_node_modules_dir: resolve_node_modules_dir(&config_file),
      config_file: WithCanonicalizedSpecifier {
        canonicalized_specifier: config_file
          .specifier
          .to_file_path()
          .ok()
          .and_then(|p| canonicalize_path_maybe_not_exists(&p).ok())
          .and_then(|p| ModuleSpecifier::from_file_path(p).ok())
          .unwrap_or_else(|| config_file.specifier.clone()),
        file: config_file,
      },
    });
  }

  pub fn workspace_settings(&self) -> &WorkspaceSettings {
    &self.settings.workspace
  }

  /// Set the workspace settings directly, which occurs during initialization
  /// and when the client does not support workspace configuration requests
  pub fn set_workspace_settings(&mut self, settings: WorkspaceSettings) {
    self.settings.workspace = settings;
    // See https://github.com/denoland/vscode_deno/issues/908.
    if self.settings.workspace.enable_paths == Some(vec![]) {
      self.settings.workspace.enable_paths = None;
    }
  }

  pub fn snapshot(&self) -> Arc<ConfigSnapshot> {
    Arc::new(ConfigSnapshot {
      client_capabilities: self.client_capabilities.clone(),
      config_file: self.maybe_config_file().cloned(),
      settings: self.settings.clone(),
      workspace_folders: self.workspace_folders.clone(),
    })
  }

  pub fn has_specifier_settings(&self, specifier: &ModuleSpecifier) -> bool {
    self.settings.specifiers.contains_key(specifier)
  }

  pub fn specifier_enabled(&self, specifier: &ModuleSpecifier) -> bool {
    specifier_enabled(
      specifier,
      self.maybe_config_file(),
      &self.settings,
      &self.workspace_folders,
    )
  }

  pub fn specifier_enabled_for_test(
    &self,
    specifier: &ModuleSpecifier,
  ) -> bool {
    if let Some(cf) = self.maybe_config_file() {
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

  pub fn get_enabled_paths(&self) -> Vec<PathBuf> {
    let mut paths = vec![];
    for (workspace_uri, _) in &self.workspace_folders {
      let Ok(workspace_path) = specifier_to_file_path(workspace_uri) else {
        lsp_log!("Unable to convert uri \"{}\" to path.", workspace_uri);
        continue;
      };
      let specifier_settings = self.settings.specifiers.get(workspace_uri);
      let enable_paths = specifier_settings
        .and_then(|s| s.enable_paths.as_ref())
        .or(self.settings.workspace.enable_paths.as_ref());
      if let Some(enable_paths) = enable_paths {
        for path in enable_paths {
          paths.push(workspace_path.join(path));
        }
      } else {
        paths.push(workspace_path);
      }
    }
    paths.sort();
    paths.dedup();
    paths
  }

  pub fn get_disabled_paths(&self) -> Vec<PathBuf> {
    let mut paths = vec![];
    if let Some(cf) = self.maybe_config_file() {
      if let Some(files) = cf.to_files_config().ok().flatten() {
        for path in files.exclude {
          paths.push(path);
        }
      }
    }
    let root_enable = self
      .settings
      .workspace
      .enable
      .unwrap_or(self.has_config_file());
    for (workspace_uri, _) in &self.workspace_folders {
      let Ok(workspace_path) = specifier_to_file_path(workspace_uri) else {
        lsp_log!("Unable to convert uri \"{}\" to path.", workspace_uri);
        continue;
      };
      let specifier_settings = self.settings.specifiers.get(workspace_uri);
      let enable = specifier_settings
        .and_then(|s| s.enable)
        .unwrap_or(root_enable);
      if enable {
        let disable_paths = specifier_settings
          .map(|s| &s.disable_paths)
          .unwrap_or(&self.settings.workspace.disable_paths);
        for path in disable_paths {
          paths.push(workspace_path.join(path));
        }
      } else {
        paths.push(workspace_path);
      }
    }
    paths.sort();
    paths.dedup();
    paths
  }

  pub fn specifier_code_lens_test(&self, specifier: &ModuleSpecifier) -> bool {
    let value = self
      .settings
      .specifiers
      .get(specifier)
      .map(|settings| settings.code_lens.test)
      .unwrap_or_else(|| self.settings.workspace.code_lens.test);
    value
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

  pub fn get_specifiers(&self) -> Vec<ModuleSpecifier> {
    self.settings.specifiers.keys().cloned().collect()
  }

  pub fn set_specifier_settings(
    &mut self,
    specifier: ModuleSpecifier,
    mut settings: SpecifierSettings,
  ) -> bool {
    // See https://github.com/denoland/vscode_deno/issues/908.
    if settings.enable_paths == Some(vec![]) {
      settings.enable_paths = None;
    }

    if let Some(existing) = self.settings.specifiers.get(&specifier) {
      if *existing == settings {
        return false;
      }
    }

    self.settings.specifiers.insert(specifier, settings);
    true
  }
}

fn specifier_enabled(
  specifier: &Url,
  config_file: Option<&ConfigFile>,
  settings: &Settings,
  workspace_folders: &Vec<(Url, lsp::WorkspaceFolder)>,
) -> bool {
  if let Some(cf) = config_file {
    if let Some(files) = cf.to_files_config().ok().flatten() {
      if !files.matches_specifier(specifier) {
        return false;
      }
    }
  }

  let root_enable = settings.workspace.enable.unwrap_or(config_file.is_some());
  if let Some(settings) = settings.specifiers.get(specifier) {
    // TODO(nayeemrmn): We don't know from where to resolve path lists in this
    // case. If they're detected, instead defer to workspace scopes.
    if settings.enable_paths.is_none() && settings.disable_paths.is_empty() {
      return settings.enable.unwrap_or(root_enable);
    }
  }
  let Ok(path) = specifier_to_file_path(specifier) else {
    // Non-file URLs are not disabled by these settings.
    return true;
  };
  for (workspace_uri, _) in workspace_folders {
    let Ok(workspace_path) = specifier_to_file_path(workspace_uri) else {
      lsp_log!("Unable to convert uri \"{}\" to path.", workspace_uri);
      continue;
    };
    if path.starts_with(&workspace_path) {
      let specifier_settings = settings.specifiers.get(workspace_uri);
      let disable_paths = specifier_settings
        .map(|s| &s.disable_paths)
        .unwrap_or(&settings.workspace.disable_paths);
      let resolved_disable_paths = disable_paths
        .iter()
        .map(|p| workspace_path.join(p))
        .collect::<Vec<_>>();
      let enable_paths = specifier_settings
        .and_then(|s| s.enable_paths.as_ref())
        .or(settings.workspace.enable_paths.as_ref());
      if let Some(enable_paths) = enable_paths {
        for enable_path in enable_paths {
          let enable_path = workspace_path.join(enable_path);
          if path.starts_with(&enable_path)
            && !resolved_disable_paths.iter().any(|p| path.starts_with(p))
          {
            return true;
          }
        }
        return false;
      } else {
        return specifier_settings
          .and_then(|s| s.enable)
          .unwrap_or(root_enable)
          && !resolved_disable_paths.iter().any(|p| path.starts_with(p));
      }
    }
  }
  root_enable
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
        lsp_log!("  Resolved lock file: \"{}\"", specifier);
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
    config.set_workspace_settings(workspace_settings);
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
    config.settings.workspace.enable = Some(true);
    config.settings.workspace.enable_paths =
      Some(vec!["mod1.ts".to_string(), "mod2.ts".to_string()]);
    config.settings.workspace.disable_paths = vec!["mod2.ts".to_string()];

    assert!(config.specifier_enabled(&root_uri.join("mod1.ts").unwrap()));
    assert!(!config.specifier_enabled(&root_uri.join("mod2.ts").unwrap()));
    assert!(!config.specifier_enabled(&root_uri.join("mod3.ts").unwrap()));
  }

  #[test]
  fn test_set_workspace_settings_defaults() {
    let mut config = Config::new();
    config.set_workspace_settings(serde_json::from_value(json!({})).unwrap());
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
    );
    assert_eq!(
      config.workspace_settings().clone(),
      WorkspaceSettings::default()
    );
  }

  #[test]
  fn config_get_enabled_paths() {
    let mut config = Config::new();
    config.workspace_folders = vec![
      (
        Url::parse("file:///root1/").unwrap(),
        lsp::WorkspaceFolder {
          uri: Url::parse("file:///root1/").unwrap(),
          name: "1".to_string(),
        },
      ),
      (
        Url::parse("file:///root2/").unwrap(),
        lsp::WorkspaceFolder {
          uri: Url::parse("file:///root2/").unwrap(),
          name: "2".to_string(),
        },
      ),
      (
        Url::parse("file:///root3/").unwrap(),
        lsp::WorkspaceFolder {
          uri: Url::parse("file:///root3/").unwrap(),
          name: "3".to_string(),
        },
      ),
    ];
    config.set_specifier_settings(
      Url::parse("file:///root1/").unwrap(),
      SpecifierSettings {
        enable_paths: Some(vec![
          "sub_dir".to_string(),
          "sub_dir/other".to_string(),
          "test.ts".to_string(),
        ]),
        ..Default::default()
      },
    );
    config.set_specifier_settings(
      Url::parse("file:///root2/").unwrap(),
      SpecifierSettings {
        enable_paths: Some(vec!["other.ts".to_string()]),
        ..Default::default()
      },
    );
    config.set_specifier_settings(
      Url::parse("file:///root3/").unwrap(),
      SpecifierSettings {
        enable: Some(true),
        ..Default::default()
      },
    );

    assert_eq!(
      config.get_enabled_paths(),
      vec![
        PathBuf::from("/root1/sub_dir"),
        PathBuf::from("/root1/sub_dir/other"),
        PathBuf::from("/root1/test.ts"),
        PathBuf::from("/root2/other.ts"),
        PathBuf::from("/root3/"),
      ]
    );
  }

  #[test]
  fn config_enable_via_config_file_detection() {
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_root(root_uri.clone());
    config.settings.workspace.enable = None;
    assert!(!config.specifier_enabled(&root_uri));

    config.set_config_file(
      ConfigFile::new("{}", root_uri.join("deno.json").unwrap()).unwrap(),
    );
    assert!(config.specifier_enabled(&root_uri));
  }

  // Regression test for https://github.com/denoland/vscode_deno/issues/917.
  #[test]
  fn config_specifier_enabled_matches_by_path_component() {
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_root(root_uri.clone());
    config.settings.workspace.enable_paths = Some(vec!["mo".to_string()]);
    assert!(!config.specifier_enabled(&root_uri.join("mod.ts").unwrap()));
  }

  #[test]
  fn config_specifier_enabled_for_test() {
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_root(root_uri.clone());
    config.settings.workspace.enable = Some(true);

    config.settings.workspace.enable_paths =
      Some(vec!["mod1.ts".to_string(), "mod2.ts".to_string()]);
    config.settings.workspace.disable_paths = vec!["mod2.ts".to_string()];
    assert!(
      config.specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod3.ts").unwrap())
    );
    config.settings.workspace.enable_paths = None;

    config.set_config_file(
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
    );
    assert!(
      config.specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod3.ts").unwrap())
    );

    config.set_config_file(
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
    );
    assert!(
      config.specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap())
    );

    config.set_config_file(
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
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap())
    );
    assert!(
      !config.specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap())
    );
  }

  #[test]
  fn config_snapshot_specifier_enabled_for_test() {
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_root(root_uri.clone());
    config.settings.workspace.enable = Some(true);
    config.set_config_file(
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
    );
    let config_snapshot = config.snapshot();
    assert!(config_snapshot
      .specifier_enabled_for_test(&root_uri.join("mod1.ts").unwrap()));
    assert!(!config_snapshot
      .specifier_enabled_for_test(&root_uri.join("mod2.ts").unwrap()));
    assert!(!config_snapshot
      .specifier_enabled_for_test(&root_uri.join("mod3.ts").unwrap()));
  }
}
