// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::logging::lsp_log;
use crate::args::discover_npmrc;
use crate::args::read_lockfile_at_path;
use crate::args::ConfigFile;
use crate::args::FmtOptions;
use crate::args::LintOptions;
use crate::args::DENO_FUTURE;
use crate::cache::FastInsecureHasher;
use crate::file_fetcher::FileFetcher;
use crate::lsp::logging::lsp_warn;
use crate::tools::lint::get_configured_rules;
use crate::tools::lint::ConfiguredRules;
use crate::util::fs::canonicalize_path_maybe_not_exists;
use deno_ast::MediaType;
use deno_config::FmtOptionsConfig;
use deno_config::TsConfig;
use deno_core::anyhow::anyhow;
use deno_core::normalize_path;
use deno_core::parking_lot::Mutex;
use deno_core::serde::de::DeserializeOwned;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use deno_lint::linter::LintConfig;
use deno_lockfile::Lockfile;
use deno_npm::npm_rc::ResolvedNpmRc;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::fs_util::specifier_to_file_path;
use deno_semver::package::PackageNv;
use deno_semver::Version;
use import_map::ImportMap;
use lsp::Url;
use lsp_types::ClientCapabilities;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::lsp_types as lsp;

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
  pub unscoped: WorkspaceSettings,
  pub by_workspace_folder: BTreeMap<ModuleSpecifier, Option<WorkspaceSettings>>,
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
  pub client_capabilities: ClientCapabilities,
  pub settings: Settings,
  pub workspace_folders: Vec<(ModuleSpecifier, lsp::WorkspaceFolder)>,
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
    self.settings.by_workspace_folder =
      folders.iter().map(|(s, _)| (s.clone(), None)).collect();
    self.settings.first_folder = folders.first().map(|(s, _)| s.clone());
    self.workspace_folders = folders;
  }

  pub fn set_workspace_settings(
    &mut self,
    unscoped: WorkspaceSettings,
    folder_settings: Vec<(ModuleSpecifier, WorkspaceSettings)>,
  ) {
    self.settings.unscoped = unscoped;
    for (folder_uri, settings) in folder_settings.into_iter() {
      if let Some(settings_) =
        self.settings.by_workspace_folder.get_mut(&folder_uri)
      {
        *settings_ = Some(settings);
      }
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
    let config_file = self.tree.config_file_for_specifier(specifier);
    if let Some(cf) = config_file {
      if let Ok(files) = cf.to_files_config() {
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
    self.client_capabilities = client_capabilities;
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
        "jsx": "react",
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

#[derive(Debug, Clone)]
pub struct LspWorkspaceConfig {
  pub members: Vec<ModuleSpecifier>,
}

#[derive(Debug, Clone)]
pub struct LspPackageConfig {
  pub nv: PackageNv,
  pub exports: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigWatchedFileType {
  DenoJson,
  Lockfile,
  PackageJson,
  ImportMap,
}

/// Contains the config file and dependent information.
#[derive(Debug, Clone)]
pub struct ConfigData {
  pub scope: ModuleSpecifier,
  pub config_file: Option<Arc<ConfigFile>>,
  pub fmt_options: Arc<FmtOptions>,
  pub lint_options: Arc<LintOptions>,
  pub lint_config: LintConfig,
  pub lint_rules: Arc<ConfiguredRules>,
  pub ts_config: Arc<LspTsConfig>,
  pub byonm: bool,
  pub node_modules_dir: Option<PathBuf>,
  pub vendor_dir: Option<PathBuf>,
  pub lockfile: Option<Arc<Mutex<Lockfile>>>,
  pub package_json: Option<Arc<PackageJson>>,
  pub npmrc: Option<Arc<ResolvedNpmRc>>,
  pub import_map: Option<Arc<ImportMap>>,
  pub import_map_from_settings: bool,
  pub package_config: Option<Arc<LspPackageConfig>>,
  pub is_workspace_root: bool,
  /// Workspace member directories. For a workspace root this will be a list of
  /// members. For a member this will be the same list, representing self and
  /// siblings. For a solitary package this will be `vec![self.scope]`. These
  /// are the list of packages to override with local resolutions for this
  /// config scope.
  pub workspace_members: Arc<Vec<ModuleSpecifier>>,
  watched_files: HashMap<ModuleSpecifier, ConfigWatchedFileType>,
}

impl ConfigData {
  async fn load(
    config_file_specifier: Option<&ModuleSpecifier>,
    scope: &ModuleSpecifier,
    workspace_root: Option<(&ModuleSpecifier, &ConfigData)>,
    settings: &Settings,
    file_fetcher: Option<&Arc<FileFetcher>>,
  ) -> Self {
    if let Some(specifier) = config_file_specifier {
      match ConfigFile::from_specifier(
        specifier.clone(),
        &deno_config::ParseOptions::default(),
      ) {
        Ok(config_file) => {
          lsp_log!(
            "  Resolved Deno configuration file: \"{}\"",
            config_file.specifier.as_str()
          );
          Self::load_inner(
            Some(config_file),
            scope,
            workspace_root,
            settings,
            file_fetcher,
          )
          .await
        }
        Err(err) => {
          lsp_warn!(
            "  Couldn't read Deno configuration file \"{}\": {}",
            specifier.as_str(),
            err
          );
          let mut data = Self::load_inner(
            None,
            scope,
            workspace_root,
            settings,
            file_fetcher,
          )
          .await;
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
      Self::load_inner(None, scope, workspace_root, settings, file_fetcher)
        .await
    }
  }

  async fn load_inner(
    config_file: Option<ConfigFile>,
    scope: &ModuleSpecifier,
    workspace_root: Option<(&ModuleSpecifier, &ConfigData)>,
    settings: &Settings,
    file_fetcher: Option<&Arc<FileFetcher>>,
  ) -> Self {
    let (settings, workspace_folder) = settings.get_for_specifier(scope);
    let mut watched_files = HashMap::with_capacity(6);
    if let Some(config_file) = &config_file {
      watched_files
        .entry(config_file.specifier.clone())
        .or_insert(ConfigWatchedFileType::DenoJson);
    }
    let config_file_path = config_file
      .as_ref()
      .and_then(|c| specifier_to_file_path(&c.specifier).ok());
    let config_file_canonicalized_specifier = config_file_path
      .as_ref()
      .and_then(|p| canonicalize_path_maybe_not_exists(p).ok())
      .and_then(|p| ModuleSpecifier::from_file_path(p).ok());
    if let Some(specifier) = config_file_canonicalized_specifier {
      watched_files
        .entry(specifier)
        .or_insert(ConfigWatchedFileType::DenoJson);
    }

    let mut fmt_options = None;
    if let Some((_, workspace_data)) = workspace_root {
      let has_own_fmt_options = config_file
        .as_ref()
        .is_some_and(|config_file| config_file.json.fmt.is_some());
      if !has_own_fmt_options {
        fmt_options = Some(workspace_data.fmt_options.clone())
      }
    }
    let fmt_options = fmt_options.unwrap_or_else(|| {
      config_file
        .as_ref()
        .and_then(|config_file| {
          config_file
            .to_fmt_config()
            .and_then(|o| {
              let base_path = config_file
                .specifier
                .to_file_path()
                .map_err(|_| anyhow!("Invalid base path."))?;
              FmtOptions::resolve(o, None, &base_path)
            })
            .inspect_err(|err| {
              lsp_warn!("  Couldn't read formatter configuration: {}", err)
            })
            .ok()
        })
        .map(Arc::new)
        .unwrap_or_default()
    });

    let mut lint_options_rules = None;
    if let Some((_, workspace_data)) = workspace_root {
      let has_own_lint_options = config_file
        .as_ref()
        .is_some_and(|config_file| config_file.json.lint.is_some());
      if !has_own_lint_options {
        lint_options_rules = Some((
          workspace_data.lint_options.clone(),
          workspace_data.lint_rules.clone(),
        ))
      }
    }
    let (lint_options, lint_rules) = lint_options_rules.unwrap_or_else(|| {
      let lint_options = config_file
        .as_ref()
        .and_then(|config_file| {
          config_file
            .to_lint_config()
            .and_then(|o| {
              let base_path = config_file
                .specifier
                .to_file_path()
                .map_err(|_| anyhow!("Invalid base path."))?;
              LintOptions::resolve(o, None, &base_path)
            })
            .inspect_err(|err| {
              lsp_warn!("  Couldn't read lint configuration: {}", err)
            })
            .ok()
        })
        .map(Arc::new)
        .unwrap_or_default();
      let lint_rules = Arc::new(get_configured_rules(
        lint_options.rules.clone(),
        config_file.as_ref(),
      ));
      (lint_options, lint_rules)
    });

    let ts_config = LspTsConfig::new(config_file.as_ref());

    let lint_config = if ts_config.inner.0.get("jsx").and_then(|v| v.as_str())
      == Some("react")
    {
      let default_jsx_factory =
        ts_config.inner.0.get("jsxFactory").and_then(|v| v.as_str());
      let default_jsx_fragment_factory = ts_config
        .inner
        .0
        .get("jsxFragmentFactory")
        .and_then(|v| v.as_str());
      deno_lint::linter::LintConfig {
        default_jsx_factory: default_jsx_factory.map(String::from),
        default_jsx_fragment_factory: default_jsx_fragment_factory
          .map(String::from),
      }
    } else {
      deno_lint::linter::LintConfig {
        default_jsx_factory: None,
        default_jsx_fragment_factory: None,
      }
    };

    let vendor_dir = config_file.as_ref().and_then(|c| c.vendor_dir_path());

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
    let package_json_path = specifier_to_file_path(scope)
      .ok()
      .map(|p| p.join("package.json"));
    if let Some(path) = &package_json_path {
      if let Ok(specifier) = ModuleSpecifier::from_file_path(path) {
        watched_files
          .entry(specifier)
          .or_insert(ConfigWatchedFileType::PackageJson);
      }
      let package_json_canonicalized_specifier =
        canonicalize_path_maybe_not_exists(path)
          .ok()
          .and_then(|p| ModuleSpecifier::from_file_path(p).ok());
      if let Some(specifier) = package_json_canonicalized_specifier {
        watched_files
          .entry(specifier)
          .or_insert(ConfigWatchedFileType::PackageJson);
      }
      if let Ok(source) = std::fs::read_to_string(path) {
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
    let npmrc = discover_npmrc(package_json_path, config_file_path)
      .inspect(|(_, path)| {
        if let Some(path) = path {
          lsp_log!("  Resolved .npmrc: \"{}\"", path.display());
        }
      })
      .inspect_err(|err| {
        lsp_warn!("  Couldn't read .npmrc for \"{scope}\": {err}");
      })
      .map(|(r, _)| r)
      .ok();
    let byonm = std::env::var("DENO_UNSTABLE_BYONM").is_ok()
      || config_file
        .as_ref()
        .map(|c| c.has_unstable("byonm"))
        .unwrap_or(false)
      || (*DENO_FUTURE
        && package_json.is_some()
        && config_file
          .as_ref()
          .map(|c| c.json.node_modules_dir.is_none())
          .unwrap_or(true));
    if byonm {
      lsp_log!("  Enabled 'bring your own node_modules'.");
    }
    let node_modules_dir = config_file
      .as_ref()
      .and_then(|c| resolve_node_modules_dir(c, byonm));

    // Load import map
    let mut import_map = None;
    let mut import_map_value = None;
    let mut import_map_specifier = None;
    let mut import_map_from_settings = false;
    if let Some(config_file) = &config_file {
      if config_file.is_an_import_map() {
        import_map_value = Some(config_file.to_import_map_value_from_imports());
        import_map_specifier = Some(config_file.specifier.clone());
      } else if let Ok(Some(specifier)) = config_file.to_import_map_specifier()
      {
        import_map_specifier = Some(specifier);
      }
    }
    import_map_specifier = import_map_specifier.or_else(|| {
      let import_map_str = settings.import_map.as_ref()?;
      let specifier = Url::parse(import_map_str)
        .ok()
        .or_else(|| workspace_folder?.join(import_map_str).ok())?;
      import_map_from_settings = true;
      Some(specifier)
    });
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
          // spawn due to the lsp's `Send` requirement
          let fetch_result = deno_core::unsync::spawn({
            let file_fetcher = file_fetcher.clone();
            let specifier = specifier.clone();
            async move {
              file_fetcher
                .fetch(&specifier, &PermissionsContainer::allow_all())
                .await
            }
          })
          .await
          .unwrap();
          let value_result = fetch_result.and_then(|f| {
            serde_json::from_slice::<Value>(&f.source).map_err(|e| e.into())
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
      match import_map::parse_from_value(specifier.clone(), value) {
        Ok(result) => {
          if config_file.as_ref().map(|c| &c.specifier) == Some(&specifier) {
            lsp_log!("  Resolved import map from configuration file");
          } else {
            lsp_log!("  Resolved import map: \"{}\"", specifier.as_str());
          }
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

    let package_config = config_file.as_ref().and_then(|c| {
      Some(LspPackageConfig {
        nv: PackageNv {
          name: c.json.name.clone()?,
          version: Version::parse_standard(c.json.version.as_ref()?).ok()?,
        },
        exports: c.json.exports.clone()?,
      })
    });

    let is_workspace_root = config_file
      .as_ref()
      .is_some_and(|c| !c.json.workspaces.is_empty());
    let workspace_members = if is_workspace_root {
      Arc::new(
        config_file
          .as_ref()
          .map(|c| {
            c.json
              .workspaces
              .iter()
              .flat_map(|p| {
                let dir_specifier = c.specifier.join(p).ok()?;
                let dir_path = specifier_to_file_path(&dir_specifier).ok()?;
                Url::from_directory_path(normalize_path(dir_path)).ok()
              })
              .collect()
          })
          .unwrap_or_default(),
      )
    } else if let Some((_, workspace_data)) = workspace_root {
      workspace_data.workspace_members.clone()
    } else if config_file.as_ref().is_some_and(|c| c.json.name.is_some()) {
      Arc::new(vec![scope.clone()])
    } else {
      Arc::new(vec![])
    };

    ConfigData {
      scope: scope.clone(),
      config_file: config_file.map(Arc::new),
      fmt_options,
      lint_options,
      lint_config,
      lint_rules,
      ts_config: Arc::new(ts_config),
      byonm,
      node_modules_dir,
      vendor_dir,
      lockfile: lockfile.map(Mutex::new).map(Arc::new),
      package_json: package_json.map(Arc::new),
      npmrc,
      import_map: import_map.map(Arc::new),
      import_map_from_settings,
      package_config: package_config.map(Arc::new),
      is_workspace_root,
      workspace_members,
      watched_files,
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct ConfigTree {
  first_folder: Option<ModuleSpecifier>,
  scopes: Arc<BTreeMap<ModuleSpecifier, ConfigData>>,
}

impl ConfigTree {
  pub fn root_scope(&self) -> Option<&ModuleSpecifier> {
    self.first_folder.as_ref()
  }

  pub fn root_data(&self) -> Option<&ConfigData> {
    self.first_folder.as_ref().and_then(|s| self.scopes.get(s))
  }

  pub fn root_ts_config(&self) -> Arc<LspTsConfig> {
    self
      .root_data()
      .map(|d| d.ts_config.clone())
      .unwrap_or_default()
  }

  pub fn root_import_map(&self) -> Option<&Arc<ImportMap>> {
    self.root_data().and_then(|d| d.import_map.as_ref())
  }

  pub fn scope_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&ModuleSpecifier> {
    self
      .scopes
      .keys()
      .rfind(|s| specifier.as_str().starts_with(s.as_str()))
      .or(self.first_folder.as_ref())
  }

  pub fn data_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&ConfigData> {
    self
      .scope_for_specifier(specifier)
      .and_then(|s| self.scopes.get(s))
  }

  pub fn data_by_scope(&self) -> &Arc<BTreeMap<ModuleSpecifier, ConfigData>> {
    &self.scopes
  }

  pub fn config_file_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<&Arc<ConfigFile>> {
    self
      .data_for_specifier(specifier)
      .and_then(|d| d.config_file.as_ref())
  }

  pub fn config_files(&self) -> Vec<&Arc<ConfigFile>> {
    self
      .scopes
      .iter()
      .filter_map(|(_, d)| d.config_file.as_ref())
      .collect()
  }

  pub fn package_jsons(&self) -> Vec<&Arc<PackageJson>> {
    self
      .scopes
      .iter()
      .filter_map(|(_, d)| d.package_json.as_ref())
      .collect()
  }

  pub fn fmt_options_for_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Arc<FmtOptions> {
    self
      .data_for_specifier(specifier)
      .map(|d| d.fmt_options.clone())
      .unwrap_or_default()
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
    workspace_files: &BTreeSet<ModuleSpecifier>,
    file_fetcher: &Arc<FileFetcher>,
  ) {
    lsp_log!("Refreshing configuration tree...");
    let mut scopes = BTreeMap::new();
    for (folder_uri, ws_settings) in &settings.by_workspace_folder {
      let mut ws_settings = ws_settings.as_ref();
      if Some(folder_uri) == settings.first_folder.as_ref() {
        ws_settings = ws_settings.or(Some(&settings.unscoped));
      }
      if let Some(ws_settings) = ws_settings {
        if let Some(config_path) = &ws_settings.config {
          if let Ok(config_uri) = folder_uri.join(config_path) {
            scopes.insert(
              folder_uri.clone(),
              ConfigData::load(
                Some(&config_uri),
                folder_uri,
                None,
                settings,
                Some(file_fetcher),
              )
              .await,
            );
          }
        }
      }
    }

    for specifier in workspace_files {
      if !(specifier.path().ends_with("/deno.json")
        || specifier.path().ends_with("/deno.jsonc"))
      {
        continue;
      }
      let Ok(scope) = specifier.join(".") else {
        continue;
      };
      if scopes.contains_key(&scope) {
        continue;
      }
      let data = ConfigData::load(
        Some(specifier),
        &scope,
        None,
        settings,
        Some(file_fetcher),
      )
      .await;
      if data.is_workspace_root {
        for member_scope in data.workspace_members.iter() {
          if scopes.contains_key(member_scope) {
            continue;
          }
          let Ok(member_path) = specifier_to_file_path(member_scope) else {
            continue;
          };
          let Some(config_file_path) = Some(member_path.join("deno.json"))
            .filter(|p| p.exists())
            .or_else(|| {
              Some(member_path.join("deno.jsonc")).filter(|p| p.exists())
            })
          else {
            continue;
          };
          let Ok(config_file_specifier) = Url::from_file_path(config_file_path)
          else {
            continue;
          };
          let member_data = ConfigData::load(
            Some(&config_file_specifier),
            member_scope,
            Some((&scope, &data)),
            settings,
            Some(file_fetcher),
          )
          .await;
          scopes.insert(member_scope.clone(), member_data);
        }
      }
      scopes.insert(scope, data);
    }

    for folder_uri in settings.by_workspace_folder.keys() {
      if !scopes
        .keys()
        .any(|s| folder_uri.as_str().starts_with(s.as_str()))
      {
        scopes.insert(
          folder_uri.clone(),
          ConfigData::load(
            None,
            folder_uri,
            None,
            settings,
            Some(file_fetcher),
          )
          .await,
        );
      }
    }
    self.first_folder = settings.first_folder.clone();
    self.scopes = Arc::new(scopes);
  }

  #[cfg(test)]
  pub async fn inject_config_file(&mut self, config_file: ConfigFile) {
    let scope = config_file.specifier.join(".").unwrap();
    let data = ConfigData::load_inner(
      Some(config_file),
      &scope,
      None,
      &Default::default(),
      None,
    )
    .await;
    self.first_folder = Some(scope.clone());
    self.scopes = Arc::new([(scope, data)].into_iter().collect());
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

fn resolve_node_modules_dir(
  config_file: &ConfigFile,
  byonm: bool,
) -> Option<PathBuf> {
  // For the language server, require an explicit opt-in via the
  // `nodeModulesDir: true` setting in the deno.json file. This is to
  // reduce the chance of modifying someone's node_modules directory
  // without them having asked us to do so.
  let explicitly_disabled = config_file.json.node_modules_dir == Some(false);
  if explicitly_disabled {
    return None;
  }
  let enabled = byonm
    || config_file.json.node_modules_dir == Some(true)
    || config_file.json.vendor == Some(true);
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
  match read_lockfile_at_path(lockfile_path) {
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
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_roots(vec![root_uri.clone()]);
    config.settings.unscoped.enable = None;
    assert!(!config.specifier_enabled(&root_uri));

    config
      .tree
      .inject_config_file(
        ConfigFile::new(
          "{}",
          root_uri.join("deno.json").unwrap(),
          &deno_config::ParseOptions::default(),
        )
        .unwrap(),
      )
      .await;
    assert!(config.specifier_enabled(&root_uri));
  }

  // Regression test for https://github.com/denoland/vscode_deno/issues/917.
  #[test]
  fn config_specifier_enabled_matches_by_path_component() {
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_roots(vec![root_uri.clone()]);
    config.settings.unscoped.enable_paths = Some(vec!["mo".to_string()]);
    assert!(!config.specifier_enabled(&root_uri.join("mod.ts").unwrap()));
  }

  #[tokio::test]
  async fn config_specifier_enabled_for_test() {
    let root_uri = resolve_url("file:///root/").unwrap();
    let mut config = Config::new_with_roots(vec![root_uri.clone()]);
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
          &deno_config::ParseOptions::default(),
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
          &deno_config::ParseOptions::default(),
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
          &deno_config::ParseOptions::default(),
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
}
