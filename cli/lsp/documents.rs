// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::cache::calculate_fs_version;
use super::cache::LspCache;
use super::cache::LSP_DISALLOW_GLOBAL_TO_LOCAL_COPY;
use super::config::Config;
use super::resolver::LspResolver;
use super::testing::TestCollector;
use super::testing::TestModule;
use super::text::LineIndex;
use super::tsc;
use super::tsc::AssetDocument;

use crate::graph_util::CliJsrUrlProvider;
use deno_runtime::fs_util::specifier_to_file_path;

use dashmap::DashMap;
use deno_ast::swc::visit::VisitWith;
use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::future::Shared;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_core::ModuleSpecifier;
use deno_graph::source::ResolutionMode;
use deno_graph::Resolution;
use deno_runtime::deno_node;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;
use indexmap::IndexMap;
use indexmap::IndexSet;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::future::Future;
use std::ops::Range;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tower_lsp::lsp_types as lsp;

pub const DOCUMENT_SCHEMES: [&str; 5] =
  ["data", "blob", "file", "http", "https"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
  JavaScript,
  Jsx,
  TypeScript,
  Tsx,
  Json,
  JsonC,
  Markdown,
  Unknown,
}

impl LanguageId {
  pub fn as_extension(&self) -> Option<&'static str> {
    match self {
      LanguageId::JavaScript => Some("js"),
      LanguageId::Jsx => Some("jsx"),
      LanguageId::TypeScript => Some("ts"),
      LanguageId::Tsx => Some("tsx"),
      LanguageId::Json => Some("json"),
      LanguageId::JsonC => Some("jsonc"),
      LanguageId::Markdown => Some("md"),
      LanguageId::Unknown => None,
    }
  }

  pub fn as_content_type(&self) -> Option<&'static str> {
    match self {
      LanguageId::JavaScript => Some("application/javascript"),
      LanguageId::Jsx => Some("text/jsx"),
      LanguageId::TypeScript => Some("application/typescript"),
      LanguageId::Tsx => Some("text/tsx"),
      LanguageId::Json | LanguageId::JsonC => Some("application/json"),
      LanguageId::Markdown => Some("text/markdown"),
      LanguageId::Unknown => None,
    }
  }

  fn is_diagnosable(&self) -> bool {
    matches!(
      self,
      Self::JavaScript | Self::Jsx | Self::TypeScript | Self::Tsx
    )
  }
}

impl FromStr for LanguageId {
  type Err = AnyError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "javascript" => Ok(Self::JavaScript),
      "javascriptreact" | "jsx" => Ok(Self::Jsx),
      "typescript" => Ok(Self::TypeScript),
      "typescriptreact" | "tsx" => Ok(Self::Tsx),
      "json" => Ok(Self::Json),
      "jsonc" => Ok(Self::JsonC),
      "markdown" => Ok(Self::Markdown),
      _ => Ok(Self::Unknown),
    }
  }
}

#[derive(Debug, PartialEq, Eq)]
enum IndexValid {
  All,
  UpTo(u32),
}

impl IndexValid {
  fn covers(&self, line: u32) -> bool {
    match *self {
      IndexValid::UpTo(to) => to > line,
      IndexValid::All => true,
    }
  }
}

#[derive(Debug, Clone)]
pub enum AssetOrDocument {
  Document(Arc<Document>),
  Asset(AssetDocument),
}

impl AssetOrDocument {
  pub fn document(&self) -> Option<&Arc<Document>> {
    match self {
      AssetOrDocument::Asset(_) => None,
      AssetOrDocument::Document(doc) => Some(doc),
    }
  }

  pub fn file_referrer(&self) -> Option<&ModuleSpecifier> {
    match self {
      AssetOrDocument::Asset(_) => None,
      AssetOrDocument::Document(doc) => doc.file_referrer(),
    }
  }

  pub fn scope(&self) -> Option<&ModuleSpecifier> {
    match self {
      AssetOrDocument::Asset(asset_doc) => Some(asset_doc.specifier()),
      AssetOrDocument::Document(doc) => doc.scope(),
    }
  }

  pub fn maybe_semantic_tokens(&self) -> Option<lsp::SemanticTokens> {
    match self {
      AssetOrDocument::Asset(_) => None,
      AssetOrDocument::Document(d) => d
        .open_data
        .as_ref()
        .and_then(|d| d.maybe_semantic_tokens.lock().clone()),
    }
  }

  pub fn text(&self) -> Arc<str> {
    match self {
      AssetOrDocument::Asset(a) => a.text(),
      AssetOrDocument::Document(d) => d.text.clone(),
    }
  }

  pub fn line_index(&self) -> Arc<LineIndex> {
    match self {
      AssetOrDocument::Asset(a) => a.line_index(),
      AssetOrDocument::Document(d) => d.line_index(),
    }
  }

  pub fn maybe_navigation_tree(&self) -> Option<Arc<tsc::NavigationTree>> {
    match self {
      AssetOrDocument::Asset(a) => a.maybe_navigation_tree(),
      AssetOrDocument::Document(d) => d.maybe_navigation_tree(),
    }
  }

  pub fn media_type(&self) -> MediaType {
    match self {
      AssetOrDocument::Asset(_) => MediaType::TypeScript, // assets are always TypeScript
      AssetOrDocument::Document(d) => d.media_type(),
    }
  }

  pub fn get_maybe_dependency(
    &self,
    position: &lsp::Position,
  ) -> Option<(String, deno_graph::Dependency, deno_graph::Range)> {
    self
      .document()
      .and_then(|d| d.get_maybe_dependency(position))
  }

  pub fn maybe_parsed_source(
    &self,
  ) -> Option<&Result<deno_ast::ParsedSource, deno_ast::ParseDiagnostic>> {
    self.document().and_then(|d| d.maybe_parsed_source())
  }

  pub fn document_lsp_version(&self) -> Option<i32> {
    self.document().and_then(|d| d.maybe_lsp_version())
  }
}

type ModuleResult = Result<deno_graph::JsModule, deno_graph::ModuleGraphError>;
type ParsedSourceResult = Result<ParsedSource, deno_ast::ParseDiagnostic>;
type TestModuleFut =
  Shared<Pin<Box<dyn Future<Output = Option<Arc<TestModule>>> + Send>>>;

fn media_type_is_diagnosable(media_type: MediaType) -> bool {
  matches!(
    media_type,
    MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::Mjs
      | MediaType::Cjs
      | MediaType::TypeScript
      | MediaType::Tsx
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
  )
}

fn get_maybe_test_module_fut(
  maybe_parsed_source: Option<&ParsedSourceResult>,
  config: &Config,
) -> Option<TestModuleFut> {
  if !config.testing_api_capable() {
    return None;
  }
  let parsed_source = maybe_parsed_source?.as_ref().ok()?.clone();
  let specifier = parsed_source.specifier();
  if specifier.scheme() != "file" {
    return None;
  }
  if !media_type_is_diagnosable(parsed_source.media_type()) {
    return None;
  }
  if !config.specifier_enabled_for_test(specifier) {
    return None;
  }
  let handle = tokio::task::spawn_blocking(move || {
    let mut collector = TestCollector::new(
      parsed_source.specifier().clone(),
      parsed_source.text_info_lazy().clone(),
    );
    parsed_source.module().visit_with(&mut collector);
    Arc::new(collector.take())
  })
  .map(Result::ok)
  .boxed()
  .shared();
  Some(handle)
}

#[derive(Clone, Debug, Default)]
pub struct DocumentOpenData {
  lsp_version: i32,
  maybe_parsed_source: Option<ParsedSourceResult>,
  maybe_semantic_tokens: Arc<Mutex<Option<lsp::SemanticTokens>>>,
}

#[derive(Debug)]
pub struct Document {
  /// Contains the last-known-good set of dependencies from parsing the module.
  config: Arc<Config>,
  dependencies: Arc<IndexMap<String, deno_graph::Dependency>>,
  // TODO(nayeemrmn): This is unused, use it for scope attribution for remote
  // modules.
  file_referrer: Option<ModuleSpecifier>,
  maybe_types_dependency: Option<Arc<deno_graph::TypesDependency>>,
  maybe_fs_version: Option<String>,
  line_index: Arc<LineIndex>,
  maybe_headers: Option<HashMap<String, String>>,
  maybe_language_id: Option<LanguageId>,
  /// This is cached in a mutex so `workspace/symbol` and
  /// `textDocument/codeLens` requests don't require a write lock.
  maybe_navigation_tree: Mutex<Option<Arc<tsc::NavigationTree>>>,
  maybe_test_module_fut: Option<TestModuleFut>,
  media_type: MediaType,
  /// Present if and only if this is an open document.
  open_data: Option<DocumentOpenData>,
  resolver: Arc<LspResolver>,
  specifier: ModuleSpecifier,
  text: Arc<str>,
  text_info_cell: once_cell::sync::OnceCell<SourceTextInfo>,
}

impl Document {
  /// Open documents should have `maybe_lsp_version.is_some()`.
  #[allow(clippy::too_many_arguments)]
  fn new(
    specifier: ModuleSpecifier,
    text: Arc<str>,
    maybe_lsp_version: Option<i32>,
    maybe_language_id: Option<LanguageId>,
    maybe_headers: Option<HashMap<String, String>>,
    resolver: Arc<LspResolver>,
    config: Arc<Config>,
    cache: &Arc<LspCache>,
    file_referrer: Option<ModuleSpecifier>,
  ) -> Arc<Self> {
    let file_referrer = Some(&specifier)
      .filter(|s| cache.is_valid_file_referrer(s))
      .cloned()
      .or(file_referrer);
    let media_type = resolve_media_type(
      &specifier,
      maybe_headers.as_ref(),
      maybe_language_id,
      &resolver,
    );
    let (maybe_parsed_source, maybe_module) =
      if media_type_is_diagnosable(media_type) {
        parse_and_analyze_module(
          specifier.clone(),
          text.clone(),
          maybe_headers.as_ref(),
          media_type,
          file_referrer.as_ref(),
          &resolver,
        )
      } else {
        (None, None)
      };
    let maybe_module = maybe_module.and_then(Result::ok);
    let dependencies = maybe_module
      .as_ref()
      .map(|m| Arc::new(m.dependencies.clone()))
      .unwrap_or_default();
    let maybe_types_dependency = maybe_module
      .as_ref()
      .and_then(|m| Some(Arc::new(m.maybe_types_dependency.clone()?)));
    let line_index = Arc::new(LineIndex::new(text.as_ref()));
    let maybe_test_module_fut =
      get_maybe_test_module_fut(maybe_parsed_source.as_ref(), &config);
    Arc::new(Self {
      config,
      dependencies,
      maybe_fs_version: calculate_fs_version(
        cache,
        &specifier,
        file_referrer.as_ref(),
      ),
      file_referrer,
      maybe_types_dependency,
      line_index,
      maybe_language_id,
      maybe_headers,
      maybe_navigation_tree: Mutex::new(None),
      maybe_test_module_fut,
      media_type,
      open_data: maybe_lsp_version.map(|v| DocumentOpenData {
        lsp_version: v,
        maybe_parsed_source,
        maybe_semantic_tokens: Default::default(),
      }),
      resolver,
      specifier,
      text,
      text_info_cell: once_cell::sync::OnceCell::new(),
    })
  }

  fn with_new_config(
    &self,
    resolver: Arc<LspResolver>,
    config: Arc<Config>,
  ) -> Arc<Self> {
    let media_type = resolve_media_type(
      &self.specifier,
      self.maybe_headers.as_ref(),
      self.maybe_language_id,
      &resolver,
    );
    let dependencies;
    let maybe_types_dependency;
    let maybe_parsed_source;
    let maybe_test_module_fut;
    if media_type != self.media_type {
      let parsed_source_result =
        parse_source(self.specifier.clone(), self.text.clone(), media_type);
      let maybe_module = analyze_module(
        self.specifier.clone(),
        &parsed_source_result,
        self.maybe_headers.as_ref(),
        self.file_referrer.as_ref(),
        &resolver,
      )
      .ok();
      dependencies = maybe_module
        .as_ref()
        .map(|m| Arc::new(m.dependencies.clone()))
        .unwrap_or_default();
      maybe_types_dependency = maybe_module
        .as_ref()
        .and_then(|m| Some(Arc::new(m.maybe_types_dependency.clone()?)));
      maybe_parsed_source = Some(parsed_source_result);
      maybe_test_module_fut =
        get_maybe_test_module_fut(maybe_parsed_source.as_ref(), &config);
    } else {
      let graph_resolver =
        resolver.as_graph_resolver(self.file_referrer.as_ref());
      let npm_resolver =
        resolver.create_graph_npm_resolver(self.file_referrer.as_ref());
      dependencies = Arc::new(
        self
          .dependencies
          .iter()
          .map(|(s, d)| {
            (
              s.clone(),
              d.with_new_resolver(
                s,
                &CliJsrUrlProvider,
                Some(graph_resolver),
                Some(&npm_resolver),
              ),
            )
          })
          .collect(),
      );
      maybe_types_dependency = self.maybe_types_dependency.as_ref().map(|d| {
        Arc::new(d.with_new_resolver(
          &CliJsrUrlProvider,
          Some(graph_resolver),
          Some(&npm_resolver),
        ))
      });
      maybe_parsed_source = self.maybe_parsed_source().cloned();
      maybe_test_module_fut = self
        .maybe_test_module_fut
        .clone()
        .filter(|_| config.specifier_enabled_for_test(&self.specifier));
    }
    Arc::new(Self {
      config,
      // updated properties
      dependencies,
      file_referrer: self.file_referrer.clone(),
      maybe_types_dependency,
      maybe_navigation_tree: Mutex::new(None),
      // maintain - this should all be copies/clones
      maybe_fs_version: self.maybe_fs_version.clone(),
      line_index: self.line_index.clone(),
      maybe_headers: self.maybe_headers.clone(),
      maybe_language_id: self.maybe_language_id,
      maybe_test_module_fut,
      media_type,
      open_data: self.open_data.as_ref().map(|d| DocumentOpenData {
        lsp_version: d.lsp_version,
        maybe_parsed_source,
        // reset semantic tokens
        maybe_semantic_tokens: Default::default(),
      }),
      resolver,
      specifier: self.specifier.clone(),
      text: self.text.clone(),
      text_info_cell: once_cell::sync::OnceCell::new(),
    })
  }

  fn with_change(
    &self,
    version: i32,
    changes: Vec<lsp::TextDocumentContentChangeEvent>,
  ) -> Result<Arc<Self>, AnyError> {
    let mut content = self.text.to_string();
    let mut line_index = self.line_index.clone();
    let mut index_valid = IndexValid::All;
    for change in changes {
      if let Some(range) = change.range {
        if !index_valid.covers(range.start.line) {
          line_index = Arc::new(LineIndex::new(&content));
        }
        index_valid = IndexValid::UpTo(range.start.line);
        let range = line_index.get_text_range(range)?;
        content.replace_range(Range::<usize>::from(range), &change.text);
      } else {
        content = change.text;
        index_valid = IndexValid::UpTo(0);
      }
    }
    let text: Arc<str> = content.into();
    let media_type = self.media_type;
    let (maybe_parsed_source, maybe_module) = if self
      .maybe_language_id
      .as_ref()
      .map(|li| li.is_diagnosable())
      .unwrap_or(false)
    {
      parse_and_analyze_module(
        self.specifier.clone(),
        text.clone(),
        self.maybe_headers.as_ref(),
        media_type,
        self.file_referrer.as_ref(),
        self.resolver.as_ref(),
      )
    } else {
      (None, None)
    };
    let maybe_module = maybe_module.and_then(Result::ok);
    let dependencies = maybe_module
      .as_ref()
      .map(|m| Arc::new(m.dependencies.clone()))
      .unwrap_or_else(|| self.dependencies.clone());
    let maybe_types_dependency = maybe_module
      .as_ref()
      .and_then(|m| Some(Arc::new(m.maybe_types_dependency.clone()?)))
      .or_else(|| self.maybe_types_dependency.clone());
    let line_index = if index_valid == IndexValid::All {
      line_index
    } else {
      Arc::new(LineIndex::new(text.as_ref()))
    };
    let maybe_test_module_fut =
      get_maybe_test_module_fut(maybe_parsed_source.as_ref(), &self.config);
    Ok(Arc::new(Self {
      config: self.config.clone(),
      specifier: self.specifier.clone(),
      file_referrer: self.file_referrer.clone(),
      maybe_fs_version: self.maybe_fs_version.clone(),
      maybe_language_id: self.maybe_language_id,
      dependencies,
      maybe_types_dependency,
      text,
      text_info_cell: once_cell::sync::OnceCell::new(),
      line_index,
      maybe_headers: self.maybe_headers.clone(),
      maybe_navigation_tree: Mutex::new(None),
      maybe_test_module_fut,
      media_type,
      open_data: self.open_data.is_some().then_some(DocumentOpenData {
        lsp_version: version,
        maybe_parsed_source,
        maybe_semantic_tokens: Default::default(),
      }),
      resolver: self.resolver.clone(),
    }))
  }

  pub fn closed(&self, cache: &Arc<LspCache>) -> Arc<Self> {
    Arc::new(Self {
      config: self.config.clone(),
      specifier: self.specifier.clone(),
      file_referrer: self.file_referrer.clone(),
      maybe_fs_version: calculate_fs_version(
        cache,
        &self.specifier,
        self.file_referrer.as_ref(),
      ),
      maybe_language_id: self.maybe_language_id,
      dependencies: self.dependencies.clone(),
      maybe_types_dependency: self.maybe_types_dependency.clone(),
      text: self.text.clone(),
      text_info_cell: once_cell::sync::OnceCell::new(),
      line_index: self.line_index.clone(),
      maybe_headers: self.maybe_headers.clone(),
      maybe_navigation_tree: Mutex::new(
        self.maybe_navigation_tree.lock().clone(),
      ),
      maybe_test_module_fut: self.maybe_test_module_fut.clone(),
      media_type: self.media_type,
      open_data: None,
      resolver: self.resolver.clone(),
    })
  }

  pub fn saved(&self, cache: &Arc<LspCache>) -> Arc<Self> {
    Arc::new(Self {
      config: self.config.clone(),
      specifier: self.specifier.clone(),
      file_referrer: self.file_referrer.clone(),
      maybe_fs_version: calculate_fs_version(
        cache,
        &self.specifier,
        self.file_referrer.as_ref(),
      ),
      maybe_language_id: self.maybe_language_id,
      dependencies: self.dependencies.clone(),
      maybe_types_dependency: self.maybe_types_dependency.clone(),
      text: self.text.clone(),
      text_info_cell: once_cell::sync::OnceCell::new(),
      line_index: self.line_index.clone(),
      maybe_headers: self.maybe_headers.clone(),
      maybe_navigation_tree: Mutex::new(
        self.maybe_navigation_tree.lock().clone(),
      ),
      maybe_test_module_fut: self.maybe_test_module_fut.clone(),
      media_type: self.media_type,
      open_data: self.open_data.clone(),
      resolver: self.resolver.clone(),
    })
  }

  pub fn specifier(&self) -> &ModuleSpecifier {
    &self.specifier
  }

  pub fn file_referrer(&self) -> Option<&ModuleSpecifier> {
    self.file_referrer.as_ref()
  }

  pub fn scope(&self) -> Option<&ModuleSpecifier> {
    self
      .file_referrer
      .as_ref()
      .and_then(|r| self.config.tree.scope_for_specifier(r))
  }

  pub fn content(&self) -> &Arc<str> {
    &self.text
  }

  pub fn text_info(&self) -> &SourceTextInfo {
    // try to get the text info from the parsed source and if
    // not then create one in the cell
    self
      .maybe_parsed_source()
      .and_then(|p| p.as_ref().ok())
      .map(|p| p.text_info_lazy())
      .unwrap_or_else(|| {
        self
          .text_info_cell
          .get_or_init(|| SourceTextInfo::new(self.text.clone()))
      })
  }

  pub fn line_index(&self) -> Arc<LineIndex> {
    self.line_index.clone()
  }

  pub fn maybe_headers(&self) -> Option<&HashMap<String, String>> {
    self.maybe_headers.as_ref()
  }

  fn maybe_fs_version(&self) -> Option<&str> {
    self.maybe_fs_version.as_deref()
  }

  pub fn script_version(&self) -> String {
    match (self.maybe_fs_version(), self.maybe_lsp_version()) {
      (None, None) => "1".to_string(),
      (None, Some(lsp_version)) => format!("1+{lsp_version}"),
      (Some(fs_version), None) => fs_version.to_string(),
      (Some(fs_version), Some(lsp_version)) => {
        format!("{fs_version}+{lsp_version}")
      }
    }
  }

  pub fn is_diagnosable(&self) -> bool {
    media_type_is_diagnosable(self.media_type())
  }

  pub fn is_open(&self) -> bool {
    self.open_data.is_some()
  }

  pub fn maybe_types_dependency(&self) -> &Resolution {
    if let Some(types_dep) = self.maybe_types_dependency.as_deref() {
      &types_dep.dependency
    } else {
      &Resolution::None
    }
  }

  pub fn media_type(&self) -> MediaType {
    self.media_type
  }

  pub fn maybe_language_id(&self) -> Option<LanguageId> {
    self.maybe_language_id
  }

  /// Returns the current language server client version if any.
  pub fn maybe_lsp_version(&self) -> Option<i32> {
    self.open_data.as_ref().map(|d| d.lsp_version)
  }

  pub fn maybe_parsed_source(
    &self,
  ) -> Option<&Result<deno_ast::ParsedSource, deno_ast::ParseDiagnostic>> {
    self.open_data.as_ref()?.maybe_parsed_source.as_ref()
  }

  pub async fn maybe_test_module(&self) -> Option<Arc<TestModule>> {
    self.maybe_test_module_fut.clone()?.await
  }

  pub fn maybe_navigation_tree(&self) -> Option<Arc<tsc::NavigationTree>> {
    self.maybe_navigation_tree.lock().clone()
  }

  pub fn dependencies(&self) -> &IndexMap<String, deno_graph::Dependency> {
    self.dependencies.as_ref()
  }

  /// If the supplied position is within a dependency range, return the resolved
  /// string specifier for the dependency, the resolved dependency and the range
  /// in the source document of the specifier.
  pub fn get_maybe_dependency(
    &self,
    position: &lsp::Position,
  ) -> Option<(String, deno_graph::Dependency, deno_graph::Range)> {
    let position = deno_graph::Position {
      line: position.line as usize,
      character: position.character as usize,
    };
    self.dependencies().iter().find_map(|(s, dep)| {
      dep
        .includes(&position)
        .map(|r| (s.clone(), dep.clone(), r.clone()))
    })
  }

  pub fn cache_navigation_tree(
    &self,
    navigation_tree: Arc<tsc::NavigationTree>,
  ) {
    *self.maybe_navigation_tree.lock() = Some(navigation_tree);
  }

  pub fn cache_semantic_tokens_full(
    &self,
    semantic_tokens: lsp::SemanticTokens,
  ) {
    if let Some(open_data) = self.open_data.as_ref() {
      *open_data.maybe_semantic_tokens.lock() = Some(semantic_tokens);
    }
  }
}

fn resolve_media_type(
  specifier: &ModuleSpecifier,
  maybe_headers: Option<&HashMap<String, String>>,
  maybe_language_id: Option<LanguageId>,
  resolver: &LspResolver,
) -> MediaType {
  if resolver.in_node_modules(specifier) {
    if let Some(media_type) = resolver.node_media_type(specifier) {
      return media_type;
    }
  }

  if let Some(language_id) = maybe_language_id {
    return MediaType::from_specifier_and_content_type(
      specifier,
      language_id.as_content_type(),
    );
  }

  if maybe_headers.is_some() {
    return MediaType::from_specifier_and_headers(specifier, maybe_headers);
  }

  MediaType::from_specifier(specifier)
}

pub fn to_lsp_range(range: &deno_graph::Range) -> lsp::Range {
  lsp::Range {
    start: lsp::Position {
      line: range.start.line as u32,
      character: range.start.character as u32,
    },
    end: lsp::Position {
      line: range.end.line as u32,
      character: range.end.character as u32,
    },
  }
}

#[derive(Debug, Default)]
struct FileSystemDocuments {
  docs: DashMap<ModuleSpecifier, Arc<Document>>,
  dirty: AtomicBool,
}

impl FileSystemDocuments {
  pub fn get(
    &self,
    specifier: &ModuleSpecifier,
    resolver: &Arc<LspResolver>,
    config: &Arc<Config>,
    cache: &Arc<LspCache>,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<Arc<Document>> {
    let file_referrer = Some(specifier)
      .filter(|s| cache.is_valid_file_referrer(s))
      .or(file_referrer);
    let new_fs_version = calculate_fs_version(cache, specifier, file_referrer);
    let old_doc = self.docs.get(specifier).map(|v| v.value().clone());
    let dirty = match &old_doc {
      None => true,
      Some(old_doc) => {
        match (old_doc.maybe_fs_version(), new_fs_version.as_deref()) {
          (None, None) => {
            matches!(specifier.scheme(), "file" | "http" | "https")
          }
          (old, new) => old != new,
        }
      }
    };
    if dirty {
      // attempt to update the file on the file system
      self.refresh_document(specifier, resolver, config, cache, file_referrer)
    } else {
      old_doc
    }
  }

  /// Adds or updates a document by reading the document from the file system
  /// returning the document.
  fn refresh_document(
    &self,
    specifier: &ModuleSpecifier,
    resolver: &Arc<LspResolver>,
    config: &Arc<Config>,
    cache: &Arc<LspCache>,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<Arc<Document>> {
    let doc = if specifier.scheme() == "file" {
      let path = specifier_to_file_path(specifier).ok()?;
      let bytes = fs::read(path).ok()?;
      let content =
        deno_graph::source::decode_owned_source(specifier, bytes, None).ok()?;
      Document::new(
        specifier.clone(),
        content.into(),
        None,
        None,
        None,
        resolver.clone(),
        config.clone(),
        cache,
        file_referrer.cloned(),
      )
    } else if specifier.scheme() == "data" {
      let source = deno_graph::source::RawDataUrl::parse(specifier)
        .ok()?
        .decode()
        .ok()?;
      Document::new(
        specifier.clone(),
        source.into(),
        None,
        None,
        None,
        resolver.clone(),
        config.clone(),
        cache,
        file_referrer.cloned(),
      )
    } else {
      let http_cache = cache.for_specifier(file_referrer);
      let cache_key = http_cache.cache_item_key(specifier).ok()?;
      let bytes = http_cache
        .read_file_bytes(&cache_key, None, LSP_DISALLOW_GLOBAL_TO_LOCAL_COPY)
        .ok()??;
      let specifier_headers = http_cache.read_headers(&cache_key).ok()??;
      let (_, maybe_charset) =
        deno_graph::source::resolve_media_type_and_charset_from_headers(
          specifier,
          Some(&specifier_headers),
        );
      let content = deno_graph::source::decode_owned_source(
        specifier,
        bytes,
        maybe_charset,
      )
      .ok()?;
      let maybe_headers = Some(specifier_headers);
      Document::new(
        specifier.clone(),
        content.into(),
        None,
        None,
        maybe_headers,
        resolver.clone(),
        config.clone(),
        cache,
        file_referrer.cloned(),
      )
    };
    self.docs.insert(specifier.clone(), doc.clone());
    self.set_dirty(true);
    Some(doc)
  }

  pub fn remove_document(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Arc<Document>> {
    Some(self.docs.remove(specifier)?.1)
  }

  /// Sets the dirty flag to the provided value and returns the previous value.
  pub fn set_dirty(&self, dirty: bool) -> bool {
    self.dirty.swap(dirty, Ordering::Relaxed)
  }
}

/// Specify the documents to include on a `documents.documents(...)` call.
#[derive(Debug, Clone, Copy)]
pub enum DocumentsFilter {
  /// Includes all the documents (diagnosable & non-diagnosable, open & file system).
  All,
  /// Includes all the diagnosable documents (open & file system).
  AllDiagnosable,
  /// Includes only the diagnosable documents that are open.
  OpenDiagnosable,
}

#[derive(Debug, Default, Clone)]
pub struct Documents {
  /// The DENO_DIR that the documents looks for non-file based modules.
  cache: Arc<LspCache>,
  config: Arc<Config>,
  /// A flag that indicates that stated data is potentially invalid and needs to
  /// be recalculated before being considered valid.
  dirty: bool,
  /// A map of documents that are "open" in the language server.
  open_docs: HashMap<ModuleSpecifier, Arc<Document>>,
  /// Documents stored on the file system.
  file_system_docs: Arc<FileSystemDocuments>,
  /// A resolver that takes into account currently loaded import map and JSX
  /// settings.
  resolver: Arc<LspResolver>,
  /// The npm package requirements found in npm specifiers.
  npm_reqs_by_scope:
    Arc<BTreeMap<Option<ModuleSpecifier>, BTreeSet<PackageReq>>>,
  /// Config scopes that contain a node: specifier such that a @types/node
  /// package should be injected.
  scopes_with_node_specifier: Arc<HashSet<Option<ModuleSpecifier>>>,
}

impl Documents {
  /// "Open" a document from the perspective of the editor, meaning that
  /// requests for information from the document will come from the in-memory
  /// representation received from the language server client, versus reading
  /// information from the disk.
  pub fn open(
    &mut self,
    specifier: ModuleSpecifier,
    version: i32,
    language_id: LanguageId,
    content: Arc<str>,
    file_referrer: Option<ModuleSpecifier>,
  ) -> Arc<Document> {
    let old_doc = self.file_system_docs.remove_document(&specifier);
    self.file_system_docs.set_dirty(true);
    let file_referrer = old_doc
      .and_then(|d| d.file_referrer().cloned())
      .or(file_referrer);

    let document = Document::new(
      specifier.clone(),
      content,
      Some(version),
      Some(language_id),
      // todo(dsherret): don't we want to pass in the headers from
      // the cache for remote modules here in order to get the
      // x-typescript-types?
      None,
      self.resolver.clone(),
      self.config.clone(),
      &self.cache,
      file_referrer,
    );

    self.open_docs.insert(specifier, document.clone());
    self.dirty = true;
    document
  }

  /// Apply language server content changes to an open document.
  pub fn change(
    &mut self,
    specifier: &ModuleSpecifier,
    version: i32,
    changes: Vec<lsp::TextDocumentContentChangeEvent>,
  ) -> Result<Arc<Document>, AnyError> {
    let doc = self
      .open_docs
      .get(specifier)
      .cloned()
      .or_else(|| self.file_system_docs.remove_document(specifier))
      .map(Ok)
      .unwrap_or_else(|| {
        Err(custom_error(
          "NotFound",
          format!("The specifier \"{specifier}\" was not found."),
        ))
      })?;
    self.dirty = true;
    let doc = doc.with_change(version, changes)?;
    self.open_docs.insert(doc.specifier().clone(), doc.clone());
    Ok(doc)
  }

  pub fn save(&mut self, specifier: &ModuleSpecifier) {
    let doc = self
      .open_docs
      .get(specifier)
      .cloned()
      .or_else(|| self.file_system_docs.remove_document(specifier));
    let Some(doc) = doc else {
      return;
    };
    self.dirty = true;
    let doc = doc.saved(&self.cache);
    self.open_docs.insert(doc.specifier().clone(), doc.clone());
  }

  /// Close an open document, this essentially clears any editor state that is
  /// being held, and the document store will revert to the file system if
  /// information about the document is required.
  pub fn close(&mut self, specifier: &ModuleSpecifier) {
    if let Some(document) = self.open_docs.remove(specifier) {
      let document = document.closed(&self.cache);
      self
        .file_system_docs
        .docs
        .insert(specifier.clone(), document);

      self.dirty = true;
    }
  }

  pub fn release(&self, specifier: &ModuleSpecifier) {
    self.file_system_docs.remove_document(specifier);
    self.file_system_docs.set_dirty(true);
  }

  pub fn get_file_referrer<'a>(
    &self,
    specifier: &'a ModuleSpecifier,
  ) -> Option<Cow<'a, ModuleSpecifier>> {
    if self.is_valid_file_referrer(specifier) {
      return Some(Cow::Borrowed(specifier));
    }
    self
      .get(specifier)
      .and_then(|d| d.file_referrer().cloned().map(Cow::Owned))
  }

  pub fn is_valid_file_referrer(&self, specifier: &ModuleSpecifier) -> bool {
    self.cache.is_valid_file_referrer(specifier)
  }

  /// Return `true` if the provided specifier can be resolved to a document,
  /// otherwise `false`.
  pub fn contains_import(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> bool {
    let file_referrer = self.get_file_referrer(referrer);
    let maybe_specifier = self
      .resolver
      .as_graph_resolver(file_referrer.as_deref())
      .resolve(
        specifier,
        &deno_graph::Range {
          specifier: referrer.clone(),
          start: deno_graph::Position::zeroed(),
          end: deno_graph::Position::zeroed(),
        },
        ResolutionMode::Types,
      )
      .ok();
    if let Some(import_specifier) = maybe_specifier {
      self.exists(&import_specifier, file_referrer.as_deref())
    } else {
      false
    }
  }

  pub fn resolve_document_specifier(
    &self,
    specifier: &ModuleSpecifier,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<ModuleSpecifier> {
    let specifier = if let Ok(jsr_req_ref) =
      JsrPackageReqReference::from_specifier(specifier)
    {
      Cow::Owned(
        self
          .resolver
          .jsr_to_resource_url(&jsr_req_ref, file_referrer)?,
      )
    } else {
      Cow::Borrowed(specifier)
    };
    if !DOCUMENT_SCHEMES.contains(&specifier.scheme()) {
      return None;
    }
    self.resolver.resolve_redirects(&specifier, file_referrer)
  }

  /// Return `true` if the specifier can be resolved to a document.
  pub fn exists(
    &self,
    specifier: &ModuleSpecifier,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> bool {
    let specifier = self.resolve_document_specifier(specifier, file_referrer);
    if let Some(specifier) = specifier {
      if self.open_docs.contains_key(&specifier) {
        return true;
      }
      if specifier.scheme() == "data" {
        return true;
      }
      if specifier.scheme() == "file" {
        return specifier_to_file_path(&specifier)
          .map(|p| p.is_file())
          .unwrap_or(false);
      }
      if self.cache.for_specifier(file_referrer).contains(&specifier) {
        return true;
      }
    }
    false
  }

  pub fn npm_reqs_by_scope(
    &mut self,
  ) -> Arc<BTreeMap<Option<ModuleSpecifier>, BTreeSet<PackageReq>>> {
    self.calculate_npm_reqs_if_dirty();
    self.npm_reqs_by_scope.clone()
  }

  pub fn scopes_with_node_specifier(
    &self,
  ) -> &Arc<HashSet<Option<ModuleSpecifier>>> {
    &self.scopes_with_node_specifier
  }

  /// Return a document for the specifier.
  pub fn get(&self, specifier: &ModuleSpecifier) -> Option<Arc<Document>> {
    if let Some(document) = self.open_docs.get(specifier) {
      Some(document.clone())
    } else {
      let old_doc = self
        .file_system_docs
        .docs
        .get(specifier)
        .map(|d| d.value().clone());
      if let Some(old_doc) = old_doc {
        self.file_system_docs.get(
          specifier,
          &self.resolver,
          &self.config,
          &self.cache,
          old_doc.file_referrer(),
        )
      } else {
        None
      }
    }
  }

  /// Return a document for the specifier.
  pub fn get_or_load(
    &self,
    specifier: &ModuleSpecifier,
    referrer: &ModuleSpecifier,
  ) -> Option<Arc<Document>> {
    let file_referrer = self.get_file_referrer(referrer);
    let specifier =
      self.resolve_document_specifier(specifier, file_referrer.as_deref())?;
    if let Some(document) = self.open_docs.get(&specifier) {
      Some(document.clone())
    } else {
      self.file_system_docs.get(
        &specifier,
        &self.resolver,
        &self.config,
        &self.cache,
        file_referrer.as_deref(),
      )
    }
  }

  /// Return a collection of documents that are contained in the document store
  /// based on the provided filter.
  pub fn documents(&self, filter: DocumentsFilter) -> Vec<Arc<Document>> {
    match filter {
      DocumentsFilter::OpenDiagnosable => self
        .open_docs
        .values()
        .filter_map(|doc| {
          if doc.is_diagnosable() {
            Some(doc.clone())
          } else {
            None
          }
        })
        .collect(),
      DocumentsFilter::AllDiagnosable | DocumentsFilter::All => {
        let diagnosable_only =
          matches!(filter, DocumentsFilter::AllDiagnosable);
        // it is technically possible for a Document to end up in both the open
        // and closed documents so we need to ensure we don't return duplicates
        let mut seen_documents = HashSet::new();
        self
          .open_docs
          .values()
          .cloned()
          .chain(self.file_system_docs.docs.iter().map(|v| v.value().clone()))
          .filter_map(|doc| {
            // this prefers the open documents
            if seen_documents.insert(doc.specifier().clone())
              && (!diagnosable_only || doc.is_diagnosable())
            {
              Some(doc)
            } else {
              None
            }
          })
          .collect()
      }
    }
  }

  /// For a given set of string specifiers, resolve each one from the graph,
  /// for a given referrer. This is used to provide resolution information to
  /// tsc when type checking.
  pub fn resolve(
    &self,
    specifiers: &[String],
    referrer: &ModuleSpecifier,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Vec<Option<(ModuleSpecifier, MediaType)>> {
    let document = self.get(referrer);
    let file_referrer = document
      .as_ref()
      .and_then(|d| d.file_referrer())
      .or(file_referrer);
    let dependencies = document.as_ref().map(|d| d.dependencies());
    let mut results = Vec::new();
    for specifier in specifiers {
      if specifier.starts_with("asset:") {
        if let Ok(specifier) = ModuleSpecifier::parse(specifier) {
          let media_type = MediaType::from_specifier(&specifier);
          results.push(Some((specifier, media_type)));
        } else {
          results.push(None);
        }
      } else if let Some(dep) =
        dependencies.as_ref().and_then(|d| d.get(specifier))
      {
        if let Some(specifier) = dep.maybe_type.maybe_specifier() {
          results.push(self.resolve_dependency(
            specifier,
            referrer,
            file_referrer,
          ));
        } else if let Some(specifier) = dep.maybe_code.maybe_specifier() {
          results.push(self.resolve_dependency(
            specifier,
            referrer,
            file_referrer,
          ));
        } else {
          results.push(None);
        }
      } else if let Ok(specifier) =
        self.resolver.as_graph_resolver(file_referrer).resolve(
          specifier,
          &deno_graph::Range {
            specifier: referrer.clone(),
            start: deno_graph::Position::zeroed(),
            end: deno_graph::Position::zeroed(),
          },
          ResolutionMode::Types,
        )
      {
        results.push(self.resolve_dependency(
          &specifier,
          referrer,
          file_referrer,
        ));
      } else {
        results.push(None);
      }
    }
    results
  }

  pub fn update_config(
    &mut self,
    config: &Config,
    resolver: &Arc<LspResolver>,
    cache: &LspCache,
    workspace_files: &IndexSet<ModuleSpecifier>,
  ) {
    self.config = Arc::new(config.clone());
    self.cache = Arc::new(cache.clone());
    self.resolver = resolver.clone();
    {
      let fs_docs = &self.file_system_docs;
      // Clean up non-existent documents.
      fs_docs.docs.retain(|specifier, _| {
        let Ok(path) = specifier_to_file_path(specifier) else {
          // Remove non-file schemed docs (deps). They may not be dependencies
          // anymore after updating resolvers.
          return false;
        };
        if !config.specifier_enabled(specifier) {
          return false;
        }
        path.is_file()
      });
      let mut open_docs = std::mem::take(&mut self.open_docs);
      for doc in open_docs.values_mut() {
        if !config.specifier_enabled(doc.specifier()) {
          continue;
        }
        *doc = doc.with_new_config(self.resolver.clone(), self.config.clone());
      }
      for mut doc in self.file_system_docs.docs.iter_mut() {
        if !config.specifier_enabled(doc.specifier()) {
          continue;
        }
        *doc.value_mut() =
          doc.with_new_config(self.resolver.clone(), self.config.clone());
      }
      self.open_docs = open_docs;
      let mut preload_count = 0;
      for specifier in workspace_files {
        if !config.specifier_enabled(specifier) {
          continue;
        }
        if preload_count >= config.settings.unscoped.document_preload_limit {
          break;
        }
        preload_count += 1;
        if !self.open_docs.contains_key(specifier)
          && !fs_docs.docs.contains_key(specifier)
        {
          fs_docs.refresh_document(
            specifier,
            &self.resolver,
            &self.config,
            &self.cache,
            None,
          );
        }
      }
      fs_docs.set_dirty(true);
    }
    self.dirty = true;
  }

  /// Iterate through the documents, building a map where the key is a unique
  /// document and the value is a set of specifiers that depend on that
  /// document.
  fn calculate_npm_reqs_if_dirty(&mut self) {
    let mut npm_reqs_by_scope: BTreeMap<_, BTreeSet<_>> = Default::default();
    let mut scopes_with_specifier = HashSet::new();
    let is_fs_docs_dirty = self.file_system_docs.set_dirty(false);
    if !is_fs_docs_dirty && !self.dirty {
      return;
    }
    let mut visit_doc = |doc: &Arc<Document>| {
      let scope = doc.scope();
      let reqs = npm_reqs_by_scope.entry(scope.cloned()).or_default();
      for dependency in doc.dependencies().values() {
        if let Some(dep) = dependency.get_code() {
          if dep.scheme() == "node" {
            scopes_with_specifier.insert(scope.cloned());
          }
          if let Ok(reference) = NpmPackageReqReference::from_specifier(dep) {
            reqs.insert(reference.into_inner().req);
          }
        }
        if let Some(dep) = dependency.get_type() {
          if let Ok(reference) = NpmPackageReqReference::from_specifier(dep) {
            reqs.insert(reference.into_inner().req);
          }
        }
      }
      if let Some(dep) = doc.maybe_types_dependency().maybe_specifier() {
        if let Ok(reference) = NpmPackageReqReference::from_specifier(dep) {
          reqs.insert(reference.into_inner().req);
        }
      }
    };
    for entry in self.file_system_docs.docs.iter() {
      visit_doc(entry.value())
    }
    for doc in self.open_docs.values() {
      visit_doc(doc);
    }

    // fill the reqs from the lockfile
    for (scope, config_data) in self.config.tree.data_by_scope().as_ref() {
      if let Some(lockfile) = config_data.lockfile.as_ref() {
        let reqs = npm_reqs_by_scope.entry(Some(scope.clone())).or_default();
        let lockfile = lockfile.lock();
        for key in lockfile.content.packages.specifiers.keys() {
          if let Some(key) = key.strip_prefix("npm:") {
            if let Ok(req) = PackageReq::from_str(key) {
              reqs.insert(req);
            }
          }
        }
      }
    }

    // Ensure a @types/node package exists when any module uses a node: specifier.
    // Unlike on the command line, here we just add @types/node to the npm package
    // requirements since this won't end up in the lockfile.
    for scope in &scopes_with_specifier {
      let reqs = npm_reqs_by_scope.entry(scope.clone()).or_default();
      if !reqs.iter().any(|r| r.name == "@types/node") {
        reqs.insert(PackageReq::from_str("@types/node").unwrap());
      }
    }

    self.npm_reqs_by_scope = Arc::new(npm_reqs_by_scope);
    self.scopes_with_node_specifier = Arc::new(scopes_with_specifier);
    self.dirty = false;
  }

  pub fn resolve_dependency(
    &self,
    specifier: &ModuleSpecifier,
    referrer: &ModuleSpecifier,
    file_referrer: Option<&ModuleSpecifier>,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    if let Some(module_name) = specifier.as_str().strip_prefix("node:") {
      if deno_node::is_builtin_node_module(module_name) {
        // return itself for node: specifiers because during type checking
        // we resolve to the ambient modules in the @types/node package
        // rather than deno_std/node
        return Some((specifier.clone(), MediaType::Dts));
      }
    }
    let mut specifier = specifier.clone();
    let mut media_type = None;
    if let Ok(npm_ref) = NpmPackageReqReference::from_specifier(&specifier) {
      let (s, mt) =
        self
          .resolver
          .npm_to_file_url(&npm_ref, referrer, file_referrer)?;
      specifier = s;
      media_type = Some(mt);
    }
    let Some(doc) = self.get_or_load(&specifier, referrer) else {
      let media_type =
        media_type.unwrap_or_else(|| MediaType::from_specifier(&specifier));
      return Some((specifier, media_type));
    };
    if let Some(types) = doc.maybe_types_dependency().maybe_specifier() {
      self.resolve_dependency(types, &specifier, file_referrer)
    } else {
      Some((doc.specifier().clone(), doc.media_type()))
    }
  }
}

/// Loader that will look at the open documents.
pub struct OpenDocumentsGraphLoader<'a> {
  pub inner_loader: &'a mut dyn deno_graph::source::Loader,
  pub open_docs: &'a HashMap<ModuleSpecifier, Arc<Document>>,
}

impl<'a> OpenDocumentsGraphLoader<'a> {
  fn load_from_docs(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<deno_graph::source::LoadFuture> {
    if specifier.scheme() == "file" {
      if let Some(doc) = self.open_docs.get(specifier) {
        return Some(
          future::ready(Ok(Some(deno_graph::source::LoadResponse::Module {
            content: Arc::from(doc.content().clone()),
            specifier: doc.specifier().clone(),
            maybe_headers: None,
          })))
          .boxed_local(),
        );
      }
    }
    None
  }
}

impl<'a> deno_graph::source::Loader for OpenDocumentsGraphLoader<'a> {
  fn load(
    &self,
    specifier: &ModuleSpecifier,
    options: deno_graph::source::LoadOptions,
  ) -> deno_graph::source::LoadFuture {
    match self.load_from_docs(specifier) {
      Some(fut) => fut,
      None => self.inner_loader.load(specifier, options),
    }
  }

  fn cache_module_info(
    &self,
    specifier: &deno_ast::ModuleSpecifier,
    source: &Arc<[u8]>,
    module_info: &deno_graph::ModuleInfo,
  ) {
    self
      .inner_loader
      .cache_module_info(specifier, source, module_info)
  }
}

fn parse_and_analyze_module(
  specifier: ModuleSpecifier,
  text: Arc<str>,
  maybe_headers: Option<&HashMap<String, String>>,
  media_type: MediaType,
  file_referrer: Option<&ModuleSpecifier>,
  resolver: &LspResolver,
) -> (Option<ParsedSourceResult>, Option<ModuleResult>) {
  let parsed_source_result = parse_source(specifier.clone(), text, media_type);
  let module_result = analyze_module(
    specifier,
    &parsed_source_result,
    maybe_headers,
    file_referrer,
    resolver,
  );
  (Some(parsed_source_result), Some(module_result))
}

fn parse_source(
  specifier: ModuleSpecifier,
  text: Arc<str>,
  media_type: MediaType,
) -> ParsedSourceResult {
  deno_ast::parse_module(deno_ast::ParseParams {
    specifier,
    text,
    media_type,
    capture_tokens: true,
    scope_analysis: true,
    maybe_syntax: None,
  })
}

fn analyze_module(
  specifier: ModuleSpecifier,
  parsed_source_result: &ParsedSourceResult,
  maybe_headers: Option<&HashMap<String, String>>,
  file_referrer: Option<&ModuleSpecifier>,
  resolver: &LspResolver,
) -> ModuleResult {
  match parsed_source_result {
    Ok(parsed_source) => {
      let npm_resolver = resolver.create_graph_npm_resolver(file_referrer);
      Ok(deno_graph::parse_module_from_ast(
        deno_graph::ParseModuleFromAstOptions {
          graph_kind: deno_graph::GraphKind::TypesOnly,
          specifier,
          maybe_headers,
          parsed_source,
          // use a null file system because there's no need to bother resolving
          // dynamic imports like import(`./dir/${something}`) in the LSP
          file_system: &deno_graph::source::NullFileSystem,
          jsr_url_provider: &CliJsrUrlProvider,
          maybe_resolver: Some(resolver.as_graph_resolver(file_referrer)),
          maybe_npm_resolver: Some(&npm_resolver),
        },
      ))
    }
    Err(err) => Err(deno_graph::ModuleGraphError::ModuleError(
      deno_graph::ModuleError::ParseErr(specifier, err.clone()),
    )),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::lsp::cache::LspCache;

  use deno_config::deno_json::ConfigFile;
  use deno_config::deno_json::ConfigParseOptions;
  use deno_core::serde_json;
  use deno_core::serde_json::json;
  use pretty_assertions::assert_eq;
  use test_util::TempDir;

  async fn setup() -> (Documents, LspCache, TempDir) {
    let temp_dir = TempDir::new();
    temp_dir.create_dir_all(".deno_dir");
    let cache = LspCache::new(Some(temp_dir.uri().join(".deno_dir").unwrap()));
    let config = Config::default();
    let resolver =
      Arc::new(LspResolver::from_config(&config, &cache, None).await);
    let mut documents = Documents::default();
    documents.update_config(&config, &resolver, &cache, &Default::default());
    (documents, cache, temp_dir)
  }

  #[tokio::test]
  async fn test_documents_open_close() {
    let (mut documents, _, _) = setup().await;
    let specifier = ModuleSpecifier::parse("file:///a.ts").unwrap();
    let content = r#"import * as b from "./b.ts";
console.log(b);
"#;
    let document = documents.open(
      specifier.clone(),
      1,
      "javascript".parse().unwrap(),
      content.into(),
      None,
    );
    assert!(document.is_diagnosable());
    assert!(document.is_open());
    assert!(document.maybe_parsed_source().is_some());
    assert!(document.maybe_lsp_version().is_some());
    documents.close(&specifier);
    // We can't use `Documents::get()` here, it will look through the real FS.
    let document = documents.file_system_docs.docs.get(&specifier).unwrap();
    assert!(!document.is_open());
    assert!(document.maybe_parsed_source().is_none());
    assert!(document.maybe_lsp_version().is_none());
  }

  #[tokio::test]
  async fn test_documents_change() {
    let (mut documents, _, _) = setup().await;
    let specifier = ModuleSpecifier::parse("file:///a.ts").unwrap();
    let content = r#"import * as b from "./b.ts";
console.log(b);
"#;
    documents.open(
      specifier.clone(),
      1,
      "javascript".parse().unwrap(),
      content.into(),
      None,
    );
    documents
      .change(
        &specifier,
        2,
        vec![lsp::TextDocumentContentChangeEvent {
          range: Some(lsp::Range {
            start: lsp::Position {
              line: 1,
              character: 13,
            },
            end: lsp::Position {
              line: 1,
              character: 13,
            },
          }),
          range_length: None,
          text: r#", "hello deno""#.to_string(),
        }],
      )
      .unwrap();
    assert_eq!(
      documents.get(&specifier).unwrap().content().as_ref(),
      r#"import * as b from "./b.ts";
console.log(b, "hello deno");
"#
    );
  }

  #[tokio::test]
  async fn test_documents_ensure_no_duplicates() {
    // it should never happen that a user of this API causes this to happen,
    // but we'll guard against it anyway
    let (mut documents, _, temp_dir) = setup().await;
    let file_path = temp_dir.path().join("file.ts");
    let file_specifier = temp_dir.uri().join("file.ts").unwrap();
    file_path.write("");

    // open the document
    documents.open(
      file_specifier.clone(),
      1,
      LanguageId::TypeScript,
      "".into(),
      None,
    );

    // make a clone of the document store and close the document in that one
    let mut documents2 = documents.clone();
    documents2.close(&file_specifier);

    // At this point the document will be in both documents and the shared file system documents.
    // Now make sure that the original documents doesn't return both copies
    assert_eq!(documents.documents(DocumentsFilter::All).len(), 1);
  }

  #[tokio::test]
  async fn test_documents_refresh_dependencies_config_change() {
    // it should never happen that a user of this API causes this to happen,
    // but we'll guard against it anyway
    let (mut documents, cache, temp_dir) = setup().await;

    let file1_path = temp_dir.path().join("file1.ts");
    let file1_specifier = temp_dir.uri().join("file1.ts").unwrap();
    fs::write(&file1_path, "").unwrap();

    let file2_path = temp_dir.path().join("file2.ts");
    let file2_specifier = temp_dir.uri().join("file2.ts").unwrap();
    fs::write(&file2_path, "").unwrap();

    let file3_path = temp_dir.path().join("file3.ts");
    let file3_specifier = temp_dir.uri().join("file3.ts").unwrap();
    fs::write(&file3_path, "").unwrap();

    let mut config = Config::new_with_roots([temp_dir.uri()]);
    let workspace_settings =
      serde_json::from_str(r#"{ "enable": true }"#).unwrap();
    config.set_workspace_settings(workspace_settings, vec![]);
    let workspace_files =
      [&file1_specifier, &file2_specifier, &file3_specifier]
        .into_iter()
        .cloned()
        .collect::<IndexSet<_>>();

    // set the initial import map and point to file 2
    {
      config
        .tree
        .inject_config_file(
          ConfigFile::new(
            &json!({
              "imports": {
                "test": "./file2.ts",
              },
            })
            .to_string(),
            config.root_uri().unwrap().join("deno.json").unwrap(),
            &ConfigParseOptions::default(),
          )
          .unwrap(),
        )
        .await;

      let resolver =
        Arc::new(LspResolver::from_config(&config, &cache, None).await);
      documents.update_config(&config, &resolver, &cache, &workspace_files);

      // open the document
      let document = documents.open(
        file1_specifier.clone(),
        1,
        LanguageId::TypeScript,
        "import {} from 'test';".into(),
        None,
      );

      assert_eq!(
        document
          .dependencies()
          .get("test")
          .unwrap()
          .maybe_code
          .maybe_specifier()
          .map(ToOwned::to_owned),
        Some(file2_specifier),
      );
    }

    // now point at file 3
    {
      config
        .tree
        .inject_config_file(
          ConfigFile::new(
            &json!({
              "imports": {
                "test": "./file3.ts",
              },
            })
            .to_string(),
            config.root_uri().unwrap().join("deno.json").unwrap(),
            &ConfigParseOptions::default(),
          )
          .unwrap(),
        )
        .await;

      let resolver =
        Arc::new(LspResolver::from_config(&config, &cache, None).await);
      documents.update_config(&config, &resolver, &cache, &workspace_files);

      // check the document's dependencies
      let document = documents.get(&file1_specifier).unwrap();
      assert_eq!(
        document
          .dependencies()
          .get("test")
          .unwrap()
          .maybe_code
          .maybe_specifier()
          .map(ToOwned::to_owned),
        Some(file3_specifier),
      );
    }
  }
}
