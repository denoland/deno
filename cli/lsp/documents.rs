// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::cache::calculate_fs_version;
use super::cache::calculate_fs_version_at_path;
use super::language_server::StateNpmSnapshot;
use super::text::LineIndex;
use super::tsc;
use super::tsc::AssetDocument;

use crate::args::package_json;
use crate::args::package_json::PackageJsonDeps;
use crate::args::ConfigFile;
use crate::args::JsxImportSourceConfig;
use crate::cache::FastInsecureHasher;
use crate::cache::HttpCache;
use crate::file_fetcher::get_source_from_bytes;
use crate::file_fetcher::get_source_from_data_url;
use crate::file_fetcher::map_content_type;
use crate::lsp::logging::lsp_warn;
use crate::npm::CliNpmResolver;
use crate::resolver::CliGraphResolver;
use crate::resolver::CliGraphResolverOptions;
use crate::resolver::SloppyImportsFsEntry;
use crate::resolver::SloppyImportsResolution;
use crate::resolver::SloppyImportsResolver;
use crate::util::glob;
use crate::util::path::specifier_to_file_path;
use crate::util::text_encoding;

use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_core::url;
use deno_core::ModuleSpecifier;
use deno_graph::source::ResolutionMode;
use deno_graph::GraphImport;
use deno_graph::Resolution;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_node;
use deno_runtime::deno_node::NodeResolution;
use deno_runtime::deno_node::NodeResolutionMode;
use deno_runtime::deno_node::NodeResolver;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;
use indexmap::IndexMap;
use lsp::Url;
use once_cell::sync::Lazy;
use package_json::PackageJsonDepsProvider;
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::fs::ReadDir;
use std::ops::Range;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tower_lsp::lsp_types as lsp;

static JS_HEADERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
  ([(
    "content-type".to_string(),
    "application/javascript".to_string(),
  )])
  .into_iter()
  .collect()
});

static JSX_HEADERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
  ([("content-type".to_string(), "text/jsx".to_string())])
    .into_iter()
    .collect()
});

static TS_HEADERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
  ([(
    "content-type".to_string(),
    "application/typescript".to_string(),
  )])
  .into_iter()
  .collect()
});

static TSX_HEADERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
  ([("content-type".to_string(), "text/tsx".to_string())])
    .into_iter()
    .collect()
});

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
  pub fn as_media_type(&self) -> MediaType {
    match self {
      LanguageId::JavaScript => MediaType::JavaScript,
      LanguageId::Jsx => MediaType::Jsx,
      LanguageId::TypeScript => MediaType::TypeScript,
      LanguageId::Tsx => MediaType::Tsx,
      LanguageId::Json => MediaType::Json,
      LanguageId::JsonC => MediaType::Json,
      LanguageId::Markdown | LanguageId::Unknown => MediaType::Unknown,
    }
  }

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

  fn as_headers(&self) -> Option<&HashMap<String, String>> {
    match self {
      Self::JavaScript => Some(&JS_HEADERS),
      Self::Jsx => Some(&JSX_HEADERS),
      Self::TypeScript => Some(&TS_HEADERS),
      Self::Tsx => Some(&TSX_HEADERS),
      _ => None,
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
  Document(Document),
  Asset(AssetDocument),
}

impl AssetOrDocument {
  pub fn specifier(&self) -> &ModuleSpecifier {
    match self {
      AssetOrDocument::Asset(asset) => asset.specifier(),
      AssetOrDocument::Document(doc) => doc.specifier(),
    }
  }

  pub fn document(&self) -> Option<&Document> {
    match self {
      AssetOrDocument::Asset(_) => None,
      AssetOrDocument::Document(doc) => Some(doc),
    }
  }

  pub fn text(&self) -> Arc<str> {
    match self {
      AssetOrDocument::Asset(a) => a.text(),
      AssetOrDocument::Document(d) => d.0.text_info.text(),
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
  ) -> Option<Result<deno_ast::ParsedSource, deno_ast::Diagnostic>> {
    self.document().and_then(|d| d.maybe_parsed_source())
  }

  pub fn document_lsp_version(&self) -> Option<i32> {
    self.document().and_then(|d| d.maybe_lsp_version())
  }

  pub fn is_open(&self) -> bool {
    self.document().map(|d| d.is_open()).unwrap_or(false)
  }
}

#[derive(Debug, Default)]
struct DocumentDependencies {
  deps: IndexMap<String, deno_graph::Dependency>,
  maybe_types_dependency: Option<deno_graph::TypesDependency>,
}

impl DocumentDependencies {
  pub fn from_maybe_module(maybe_module: &Option<ModuleResult>) -> Self {
    if let Some(Ok(module)) = &maybe_module {
      Self::from_module(module)
    } else {
      Self::default()
    }
  }

  pub fn from_module(module: &deno_graph::EsmModule) -> Self {
    Self {
      deps: module.dependencies.clone(),
      maybe_types_dependency: module.maybe_types_dependency.clone(),
    }
  }
}

type ModuleResult = Result<deno_graph::EsmModule, deno_graph::ModuleGraphError>;
type ParsedSourceResult = Result<ParsedSource, deno_ast::Diagnostic>;

#[derive(Debug)]
struct DocumentInner {
  /// Contains the last-known-good set of dependencies from parsing the module.
  dependencies: Arc<DocumentDependencies>,
  fs_version: String,
  line_index: Arc<LineIndex>,
  maybe_headers: Option<HashMap<String, String>>,
  maybe_language_id: Option<LanguageId>,
  maybe_lsp_version: Option<i32>,
  maybe_module: Option<ModuleResult>,
  // this is a lazily constructed value based on the state of the document,
  // so having a mutex to hold it is ok
  maybe_navigation_tree: Mutex<Option<Arc<tsc::NavigationTree>>>,
  maybe_parsed_source: Option<ParsedSourceResult>,
  specifier: ModuleSpecifier,
  text_info: SourceTextInfo,
}

#[derive(Debug, Clone)]
pub struct Document(Arc<DocumentInner>);

impl Document {
  fn new(
    specifier: ModuleSpecifier,
    fs_version: String,
    maybe_headers: Option<HashMap<String, String>>,
    text_info: SourceTextInfo,
    resolver: &dyn deno_graph::source::Resolver,
    npm_resolver: &dyn deno_graph::source::NpmResolver,
  ) -> Self {
    // we only ever do `Document::new` on on disk resources that are supposed to
    // be diagnosable, unlike `Document::open`, so it is safe to unconditionally
    // parse the module.
    let (maybe_parsed_source, maybe_module) = parse_and_analyze_module(
      &specifier,
      text_info.clone(),
      maybe_headers.as_ref(),
      resolver,
      npm_resolver,
    );
    let dependencies =
      Arc::new(DocumentDependencies::from_maybe_module(&maybe_module));
    let line_index = Arc::new(LineIndex::new(text_info.text_str()));
    Self(Arc::new(DocumentInner {
      dependencies,
      fs_version,
      line_index,
      maybe_headers,
      maybe_language_id: None,
      maybe_lsp_version: None,
      maybe_module,
      maybe_navigation_tree: Mutex::new(None),
      maybe_parsed_source,
      text_info,
      specifier,
    }))
  }

  fn maybe_with_new_resolver(
    &self,
    resolver: &dyn deno_graph::source::Resolver,
    npm_resolver: &dyn deno_graph::source::NpmResolver,
  ) -> Option<Self> {
    let parsed_source_result = match &self.0.maybe_parsed_source {
      Some(parsed_source_result) => parsed_source_result.clone(),
      None => return None, // nothing to change
    };
    let maybe_module = Some(analyze_module(
      &self.0.specifier,
      &parsed_source_result,
      self.0.maybe_headers.as_ref(),
      resolver,
      npm_resolver,
    ));
    let dependencies =
      Arc::new(DocumentDependencies::from_maybe_module(&maybe_module));
    Some(Self(Arc::new(DocumentInner {
      // updated properties
      dependencies,
      maybe_module,
      maybe_navigation_tree: Mutex::new(None),
      maybe_parsed_source: Some(parsed_source_result),
      // maintain - this should all be copies/clones
      fs_version: self.0.fs_version.clone(),
      line_index: self.0.line_index.clone(),
      maybe_headers: self.0.maybe_headers.clone(),
      maybe_language_id: self.0.maybe_language_id,
      maybe_lsp_version: self.0.maybe_lsp_version,
      text_info: self.0.text_info.clone(),
      specifier: self.0.specifier.clone(),
    })))
  }

  fn open(
    specifier: ModuleSpecifier,
    version: i32,
    language_id: LanguageId,
    content: Arc<str>,
    cache: &Arc<dyn HttpCache>,
    resolver: &dyn deno_graph::source::Resolver,
    npm_resolver: &dyn deno_graph::source::NpmResolver,
  ) -> Self {
    let maybe_headers = language_id.as_headers();
    let text_info = SourceTextInfo::new(content);
    let (maybe_parsed_source, maybe_module) = if language_id.is_diagnosable() {
      parse_and_analyze_module(
        &specifier,
        text_info.clone(),
        maybe_headers,
        resolver,
        npm_resolver,
      )
    } else {
      (None, None)
    };
    let dependencies =
      Arc::new(DocumentDependencies::from_maybe_module(&maybe_module));
    let line_index = Arc::new(LineIndex::new(text_info.text_str()));
    Self(Arc::new(DocumentInner {
      dependencies,
      fs_version: calculate_fs_version(cache, &specifier)
        .unwrap_or_else(|| "1".to_string()),
      line_index,
      maybe_language_id: Some(language_id),
      maybe_lsp_version: Some(version),
      maybe_headers: maybe_headers.map(ToOwned::to_owned),
      maybe_module,
      maybe_navigation_tree: Mutex::new(None),
      maybe_parsed_source,
      text_info,
      specifier,
    }))
  }

  fn with_change(
    &self,
    version: i32,
    changes: Vec<lsp::TextDocumentContentChangeEvent>,
    resolver: &dyn deno_graph::source::Resolver,
    npm_resolver: &dyn deno_graph::source::NpmResolver,
  ) -> Result<Document, AnyError> {
    let mut content = self.0.text_info.text_str().to_string();
    let mut line_index = self.0.line_index.clone();
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
    let text_info = SourceTextInfo::from_string(content);
    let (maybe_parsed_source, maybe_module) = if self
      .0
      .maybe_language_id
      .as_ref()
      .map(|li| li.is_diagnosable())
      .unwrap_or(false)
    {
      let maybe_headers = self
        .0
        .maybe_language_id
        .as_ref()
        .and_then(|li| li.as_headers());
      parse_and_analyze_module(
        &self.0.specifier,
        text_info.clone(),
        maybe_headers,
        resolver,
        npm_resolver,
      )
    } else {
      (None, None)
    };
    let dependencies = if let Some(Ok(module)) = &maybe_module {
      Arc::new(DocumentDependencies::from_module(module))
    } else {
      self.0.dependencies.clone() // use the last known good
    };
    let line_index = if index_valid == IndexValid::All {
      line_index
    } else {
      Arc::new(LineIndex::new(text_info.text_str()))
    };
    Ok(Document(Arc::new(DocumentInner {
      specifier: self.0.specifier.clone(),
      fs_version: self.0.fs_version.clone(),
      maybe_language_id: self.0.maybe_language_id,
      dependencies,
      text_info,
      line_index,
      maybe_headers: self.0.maybe_headers.clone(),
      maybe_module,
      maybe_parsed_source,
      maybe_lsp_version: Some(version),
      maybe_navigation_tree: Mutex::new(None),
    })))
  }

  pub fn saved(&self, cache: &Arc<dyn HttpCache>) -> Document {
    Document(Arc::new(DocumentInner {
      specifier: self.0.specifier.clone(),
      fs_version: calculate_fs_version(cache, &self.0.specifier)
        .unwrap_or_else(|| "1".to_string()),
      maybe_language_id: self.0.maybe_language_id,
      dependencies: self.0.dependencies.clone(),
      text_info: self.0.text_info.clone(),
      line_index: self.0.line_index.clone(),
      maybe_headers: self.0.maybe_headers.clone(),
      maybe_module: self.0.maybe_module.clone(),
      maybe_parsed_source: self.0.maybe_parsed_source.clone(),
      maybe_lsp_version: self.0.maybe_lsp_version,
      maybe_navigation_tree: Mutex::new(None),
    }))
  }

  pub fn specifier(&self) -> &ModuleSpecifier {
    &self.0.specifier
  }

  pub fn content(&self) -> Arc<str> {
    self.0.text_info.text()
  }

  pub fn text_info(&self) -> SourceTextInfo {
    self.0.text_info.clone()
  }

  pub fn line_index(&self) -> Arc<LineIndex> {
    self.0.line_index.clone()
  }

  fn fs_version(&self) -> &str {
    self.0.fs_version.as_str()
  }

  pub fn script_version(&self) -> String {
    self
      .maybe_lsp_version()
      .map(|v| format!("{}+{v}", self.fs_version()))
      .unwrap_or_else(|| self.fs_version().to_string())
  }

  pub fn is_diagnosable(&self) -> bool {
    matches!(
      self.media_type(),
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

  pub fn is_open(&self) -> bool {
    self.0.maybe_lsp_version.is_some()
  }

  pub fn maybe_types_dependency(&self) -> Resolution {
    if let Some(types_dep) = self.0.dependencies.maybe_types_dependency.as_ref()
    {
      types_dep.dependency.clone()
    } else {
      Resolution::None
    }
  }

  pub fn media_type(&self) -> MediaType {
    if let Some(Ok(module)) = &self.0.maybe_module {
      return module.media_type;
    }
    let specifier_media_type = MediaType::from_specifier(&self.0.specifier);
    if specifier_media_type != MediaType::Unknown {
      return specifier_media_type;
    }

    self
      .0
      .maybe_language_id
      .map(|id| id.as_media_type())
      .unwrap_or(MediaType::Unknown)
  }

  pub fn maybe_language_id(&self) -> Option<LanguageId> {
    self.0.maybe_language_id
  }

  /// Returns the current language server client version if any.
  pub fn maybe_lsp_version(&self) -> Option<i32> {
    self.0.maybe_lsp_version
  }

  fn maybe_esm_module(&self) -> Option<&ModuleResult> {
    self.0.maybe_module.as_ref()
  }

  pub fn maybe_parsed_source(
    &self,
  ) -> Option<Result<deno_ast::ParsedSource, deno_ast::Diagnostic>> {
    self.0.maybe_parsed_source.clone()
  }

  pub fn maybe_navigation_tree(&self) -> Option<Arc<tsc::NavigationTree>> {
    self.0.maybe_navigation_tree.lock().clone()
  }

  pub fn update_navigation_tree_if_version(
    &self,
    tree: Arc<tsc::NavigationTree>,
    script_version: &str,
  ) {
    // Ensure we are updating the same document that the navigation tree was
    // created for. Note: this should not be racy between the version check
    // and setting the navigation tree, because the document is immutable
    // and this is enforced by it being wrapped in an Arc.
    if self.script_version() == script_version {
      *self.0.maybe_navigation_tree.lock() = Some(tree);
    }
  }

  pub fn dependencies(&self) -> &IndexMap<String, deno_graph::Dependency> {
    &self.0.dependencies.deps
  }

  /// If the supplied position is within a dependency range, return the resolved
  /// string specifier for the dependency, the resolved dependency and the range
  /// in the source document of the specifier.
  pub fn get_maybe_dependency(
    &self,
    position: &lsp::Position,
  ) -> Option<(String, deno_graph::Dependency, deno_graph::Range)> {
    let module = self.maybe_esm_module()?.as_ref().ok()?;
    let position = deno_graph::Position {
      line: position.line as usize,
      character: position.character as usize,
    };
    module.dependencies.iter().find_map(|(s, dep)| {
      dep
        .includes(&position)
        .map(|r| (s.clone(), dep.clone(), r.clone()))
    })
  }
}

pub fn to_hover_text(result: &Resolution) -> String {
  match result {
    Resolution::Ok(resolved) => {
      let specifier = &resolved.specifier;
      match specifier.scheme() {
        "data" => "_(a data url)_".to_string(),
        "blob" => "_(a blob url)_".to_string(),
        _ => format!(
          "{}&#8203;{}",
          &specifier[..url::Position::AfterScheme],
          &specifier[url::Position::AfterScheme..],
        )
        .replace('@', "&#8203;@"),
      }
    }
    Resolution::Err(_) => "_[errored]_".to_string(),
    Resolution::None => "_[missing]_".to_string(),
  }
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

/// Recurse and collect specifiers that appear in the dependent map.
fn recurse_dependents(
  specifier: &ModuleSpecifier,
  map: &HashMap<ModuleSpecifier, HashSet<ModuleSpecifier>>,
  dependents: &mut HashSet<ModuleSpecifier>,
) {
  if let Some(deps) = map.get(specifier) {
    for dep in deps {
      if !dependents.contains(dep) {
        dependents.insert(dep.clone());
        recurse_dependents(dep, map, dependents);
      }
    }
  }
}

#[derive(Debug)]
struct RedirectResolver {
  cache: Arc<dyn HttpCache>,
  redirects: Mutex<HashMap<ModuleSpecifier, ModuleSpecifier>>,
}

impl RedirectResolver {
  pub fn new(cache: Arc<dyn HttpCache>) -> Self {
    Self {
      cache,
      redirects: Mutex::new(HashMap::new()),
    }
  }

  pub fn resolve(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let scheme = specifier.scheme();
    if !DOCUMENT_SCHEMES.contains(&scheme) {
      return None;
    }

    if scheme == "http" || scheme == "https" {
      let mut redirects = self.redirects.lock();
      if let Some(specifier) = redirects.get(specifier) {
        Some(specifier.clone())
      } else {
        let redirect = self.resolve_remote(specifier, 10)?;
        redirects.insert(specifier.clone(), redirect.clone());
        Some(redirect)
      }
    } else {
      Some(specifier.clone())
    }
  }

  fn resolve_remote(
    &self,
    specifier: &ModuleSpecifier,
    redirect_limit: usize,
  ) -> Option<ModuleSpecifier> {
    if redirect_limit > 0 {
      let cache_key = self.cache.cache_item_key(specifier).ok()?;
      let headers = self
        .cache
        .read_metadata(&cache_key)
        .ok()
        .flatten()
        .map(|m| m.headers)?;
      if let Some(location) = headers.get("location") {
        let redirect =
          deno_core::resolve_import(location, specifier.as_str()).ok()?;
        self.resolve_remote(&redirect, redirect_limit - 1)
      } else {
        Some(specifier.clone())
      }
    } else {
      None
    }
  }
}

#[derive(Debug, Default)]
struct FileSystemDocuments {
  docs: HashMap<ModuleSpecifier, Document>,
  dirty: bool,
}

impl FileSystemDocuments {
  pub fn get(
    &mut self,
    cache: &Arc<dyn HttpCache>,
    resolver: &dyn deno_graph::source::Resolver,
    specifier: &ModuleSpecifier,
    npm_resolver: &dyn deno_graph::source::NpmResolver,
  ) -> Option<Document> {
    let fs_version = if specifier.scheme() == "data" {
      Some("1".to_string())
    } else {
      calculate_fs_version(cache, specifier)
    };
    let file_system_doc = self.docs.get(specifier);
    if file_system_doc.map(|d| d.fs_version().to_string()) != fs_version {
      // attempt to update the file on the file system
      self.refresh_document(cache, resolver, specifier, npm_resolver)
    } else {
      file_system_doc.cloned()
    }
  }

  /// Adds or updates a document by reading the document from the file system
  /// returning the document.
  fn refresh_document(
    &mut self,
    cache: &Arc<dyn HttpCache>,
    resolver: &dyn deno_graph::source::Resolver,
    specifier: &ModuleSpecifier,
    npm_resolver: &dyn deno_graph::source::NpmResolver,
  ) -> Option<Document> {
    let doc = if specifier.scheme() == "file" {
      let path = specifier_to_file_path(specifier).ok()?;
      let fs_version = calculate_fs_version_at_path(&path)?;
      let bytes = fs::read(path).ok()?;
      let maybe_charset =
        Some(text_encoding::detect_charset(&bytes).to_string());
      let content = get_source_from_bytes(bytes, maybe_charset).ok()?;
      Document::new(
        specifier.clone(),
        fs_version,
        None,
        SourceTextInfo::from_string(content),
        resolver,
        npm_resolver,
      )
    } else if specifier.scheme() == "data" {
      let (source, _) = get_source_from_data_url(specifier).ok()?;
      Document::new(
        specifier.clone(),
        "1".to_string(),
        None,
        SourceTextInfo::from_string(source),
        resolver,
        npm_resolver,
      )
    } else {
      let fs_version = calculate_fs_version(cache, specifier)?;
      let cache_key = cache.cache_item_key(specifier).ok()?;
      let bytes = cache.read_file_bytes(&cache_key).ok()??;
      let specifier_metadata = cache.read_metadata(&cache_key).ok()??;
      let maybe_content_type = specifier_metadata.headers.get("content-type");
      let (_, maybe_charset) = map_content_type(specifier, maybe_content_type);
      let maybe_headers = Some(specifier_metadata.headers);
      let content = get_source_from_bytes(bytes, maybe_charset).ok()?;
      Document::new(
        specifier.clone(),
        fs_version,
        maybe_headers,
        SourceTextInfo::from_string(content),
        resolver,
        npm_resolver,
      )
    };
    self.dirty = true;
    self.docs.insert(specifier.clone(), doc.clone());
    Some(doc)
  }
}

pub struct UpdateDocumentConfigOptions<'a> {
  pub enabled_paths: Vec<PathBuf>,
  pub disabled_paths: Vec<PathBuf>,
  pub document_preload_limit: usize,
  pub maybe_import_map: Option<Arc<import_map::ImportMap>>,
  pub maybe_config_file: Option<&'a ConfigFile>,
  pub maybe_package_json: Option<&'a PackageJson>,
  pub node_resolver: Option<Arc<NodeResolver>>,
  pub npm_resolver: Option<Arc<dyn CliNpmResolver>>,
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

#[derive(Debug, Clone)]
pub struct Documents {
  /// The DENO_DIR that the documents looks for non-file based modules.
  cache: Arc<dyn HttpCache>,
  /// A flag that indicates that stated data is potentially invalid and needs to
  /// be recalculated before being considered valid.
  dirty: bool,
  /// A map where the key is a specifier and the value is a set of specifiers
  /// that depend on the key.
  dependents_map: Arc<HashMap<ModuleSpecifier, HashSet<ModuleSpecifier>>>,
  /// A map of documents that are "open" in the language server.
  open_docs: HashMap<ModuleSpecifier, Document>,
  /// Documents stored on the file system.
  file_system_docs: Arc<Mutex<FileSystemDocuments>>,
  /// Hash of the config used for resolution. When the hash changes we update
  /// dependencies.
  resolver_config_hash: u64,
  /// Any imports to the context supplied by configuration files. This is like
  /// the imports into the a module graph in CLI.
  imports: Arc<IndexMap<ModuleSpecifier, GraphImport>>,
  /// A resolver that takes into account currently loaded import map and JSX
  /// settings.
  resolver: Arc<CliGraphResolver>,
  /// The npm package requirements found in npm specifiers.
  npm_specifier_reqs: Arc<Vec<PackageReq>>,
  /// Gets if any document had a node: specifier such that a @types/node package
  /// should be injected.
  has_injected_types_node_package: bool,
  /// Resolves a specifier to its final redirected to specifier.
  redirect_resolver: Arc<RedirectResolver>,
  /// If --unstable-sloppy-imports is enabled.
  unstable_sloppy_imports: bool,
}

impl Documents {
  pub fn new(cache: Arc<dyn HttpCache>) -> Self {
    Self {
      cache: cache.clone(),
      dirty: true,
      dependents_map: Default::default(),
      open_docs: HashMap::default(),
      file_system_docs: Default::default(),
      resolver_config_hash: 0,
      imports: Default::default(),
      resolver: Arc::new(CliGraphResolver::new(CliGraphResolverOptions {
        fs: Arc::new(RealFs),
        node_resolver: None,
        npm_resolver: None,
        cjs_resolutions: None,
        package_json_deps_provider: Arc::new(PackageJsonDepsProvider::default()),
        maybe_jsx_import_source_config: None,
        maybe_import_map: None,
        maybe_vendor_dir: None,
        bare_node_builtins_enabled: false,
        sloppy_imports_resolver: None,
      })),
      npm_specifier_reqs: Default::default(),
      has_injected_types_node_package: false,
      redirect_resolver: Arc::new(RedirectResolver::new(cache)),
      unstable_sloppy_imports: false,
    }
  }

  pub fn module_graph_imports(&self) -> impl Iterator<Item = &ModuleSpecifier> {
    self
      .imports
      .values()
      .flat_map(|i| i.dependencies.values())
      .flat_map(|value| value.get_type().or_else(|| value.get_code()))
  }

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
  ) -> Document {
    let resolver = self.get_resolver();
    let npm_resolver = self.get_npm_resolver();
    let document = Document::open(
      specifier.clone(),
      version,
      language_id,
      content,
      &self.cache,
      resolver,
      npm_resolver,
    );
    let mut file_system_docs = self.file_system_docs.lock();
    file_system_docs.docs.remove(&specifier);
    file_system_docs.dirty = true;
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
  ) -> Result<Document, AnyError> {
    let doc = self
      .open_docs
      .get(specifier)
      .cloned()
      .or_else(|| {
        let mut file_system_docs = self.file_system_docs.lock();
        file_system_docs.docs.remove(specifier)
      })
      .map(Ok)
      .unwrap_or_else(|| {
        Err(custom_error(
          "NotFound",
          format!("The specifier \"{specifier}\" was not found."),
        ))
      })?;
    self.dirty = true;
    let doc = doc.with_change(
      version,
      changes,
      self.get_resolver(),
      self.get_npm_resolver(),
    )?;
    self.open_docs.insert(doc.specifier().clone(), doc.clone());
    Ok(doc)
  }

  pub fn save(&mut self, specifier: &ModuleSpecifier) {
    let doc = self.open_docs.get(specifier).cloned().or_else(|| {
      let mut file_system_docs = self.file_system_docs.lock();
      file_system_docs.docs.remove(specifier)
    });
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
  pub fn close(&mut self, specifier: &ModuleSpecifier) -> Result<(), AnyError> {
    if let Some(document) = self.open_docs.remove(specifier) {
      let mut file_system_docs = self.file_system_docs.lock();
      file_system_docs.docs.insert(specifier.clone(), document);
      self.dirty = true;
    }
    Ok(())
  }

  /// Return `true` if the provided specifier can be resolved to a document,
  /// otherwise `false`.
  pub fn contains_import(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> bool {
    let maybe_specifier = self
      .get_resolver()
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
      self.exists(&import_specifier)
    } else {
      false
    }
  }

  pub fn resolve_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    if self.unstable_sloppy_imports && specifier.scheme() == "file" {
      Some(
        self
          .resolve_unstable_sloppy_import(specifier)
          .into_specifier()
          .into_owned(),
      )
    } else {
      self.redirect_resolver.resolve(specifier)
    }
  }

  fn resolve_unstable_sloppy_import<'a>(
    &self,
    specifier: &'a ModuleSpecifier,
  ) -> SloppyImportsResolution<'a> {
    SloppyImportsResolver::resolve_with_stat_sync(specifier, |path| {
      if let Ok(specifier) = ModuleSpecifier::from_file_path(path) {
        if self.open_docs.contains_key(&specifier)
          || self.cache.contains(&specifier)
        {
          return Some(SloppyImportsFsEntry::File);
        }
      }
      path.metadata().ok().and_then(|m| {
        if m.is_file() {
          Some(SloppyImportsFsEntry::File)
        } else if m.is_dir() {
          Some(SloppyImportsFsEntry::Dir)
        } else {
          None
        }
      })
    })
  }

  /// Return `true` if the specifier can be resolved to a document.
  pub fn exists(&self, specifier: &ModuleSpecifier) -> bool {
    let specifier = self.resolve_specifier(specifier);
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
      if self.cache.contains(&specifier) {
        return true;
      }
    }
    false
  }

  /// Return an array of specifiers, if any, that are dependent upon the
  /// supplied specifier. This is used to determine invalidation of diagnostics
  /// when a module has been changed.
  pub fn dependents(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Vec<ModuleSpecifier> {
    self.calculate_dependents_if_dirty();
    let mut dependents = HashSet::new();
    if let Some(specifier) = self.resolve_specifier(specifier) {
      recurse_dependents(&specifier, &self.dependents_map, &mut dependents);
      dependents.into_iter().collect()
    } else {
      vec![]
    }
  }

  /// Returns a collection of npm package requirements.
  pub fn npm_package_reqs(&mut self) -> Arc<Vec<PackageReq>> {
    self.calculate_dependents_if_dirty();
    self.npm_specifier_reqs.clone()
  }

  /// Returns if a @types/node package was injected into the npm
  /// resolver based on the state of the documents.
  pub fn has_injected_types_node_package(&self) -> bool {
    self.has_injected_types_node_package
  }

  /// Return a document for the specifier.
  pub fn get(&self, original_specifier: &ModuleSpecifier) -> Option<Document> {
    let specifier = self.resolve_specifier(original_specifier)?;
    if let Some(document) = self.open_docs.get(&specifier) {
      Some(document.clone())
    } else {
      let mut file_system_docs = self.file_system_docs.lock();
      file_system_docs.get(
        &self.cache,
        self.get_resolver(),
        &specifier,
        self.get_npm_resolver(),
      )
    }
  }

  /// Return a collection of documents that are contained in the document store
  /// based on the provided filter.
  pub fn documents(&self, filter: DocumentsFilter) -> Vec<Document> {
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
        let file_system_docs = self.file_system_docs.lock();
        self
          .open_docs
          .values()
          .chain(file_system_docs.docs.values())
          .filter_map(|doc| {
            // this prefers the open documents
            if seen_documents.insert(doc.specifier().clone())
              && (!diagnosable_only || doc.is_diagnosable())
            {
              Some(doc.clone())
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
    specifiers: Vec<String>,
    referrer_doc: &AssetOrDocument,
    maybe_npm: Option<&StateNpmSnapshot>,
  ) -> Vec<Option<(ModuleSpecifier, MediaType)>> {
    let referrer = referrer_doc.specifier();
    let dependencies = match referrer_doc {
      AssetOrDocument::Asset(_) => None,
      AssetOrDocument::Document(doc) => Some(doc.0.dependencies.clone()),
    };
    let mut results = Vec::new();
    for specifier in specifiers {
      if let Some(npm) = maybe_npm {
        if npm.node_resolver.in_npm_package(referrer) {
          // we're in an npm package, so use node resolution
          results.push(Some(NodeResolution::into_specifier_and_media_type(
            npm
              .node_resolver
              .resolve(
                &specifier,
                referrer,
                NodeResolutionMode::Types,
                &PermissionsContainer::allow_all(),
              )
              .ok()
              .flatten(),
          )));
          continue;
        }
      }
      if specifier.starts_with("asset:") {
        if let Ok(specifier) = ModuleSpecifier::parse(&specifier) {
          let media_type = MediaType::from_specifier(&specifier);
          results.push(Some((specifier, media_type)));
        } else {
          results.push(None);
        }
      } else if let Some(dep) =
        dependencies.as_ref().and_then(|d| d.deps.get(&specifier))
      {
        if let Some(specifier) = dep.maybe_type.maybe_specifier() {
          results.push(self.resolve_dependency(specifier, maybe_npm, referrer));
        } else if let Some(specifier) = dep.maybe_code.maybe_specifier() {
          results.push(self.resolve_dependency(specifier, maybe_npm, referrer));
        } else {
          results.push(None);
        }
      } else if let Some(specifier) = self
        .resolve_imports_dependency(&specifier)
        .and_then(|r| r.maybe_specifier())
      {
        results.push(self.resolve_dependency(specifier, maybe_npm, referrer));
      } else if let Ok(npm_req_ref) =
        NpmPackageReqReference::from_str(&specifier)
      {
        results.push(node_resolve_npm_req_ref(
          npm_req_ref,
          maybe_npm,
          referrer,
        ));
      } else {
        results.push(None);
      }
    }
    results
  }

  /// Update the location of the on disk cache for the document store.
  pub fn set_cache(&mut self, cache: Arc<dyn HttpCache>) {
    // TODO update resolved dependencies?
    self.cache = cache.clone();
    self.redirect_resolver = Arc::new(RedirectResolver::new(cache));
    self.dirty = true;
  }

  /// Tries to cache a navigation tree that is associated with the provided specifier
  /// if the document stored has the same script version.
  pub fn try_cache_navigation_tree(
    &self,
    specifier: &ModuleSpecifier,
    script_version: &str,
    navigation_tree: Arc<tsc::NavigationTree>,
  ) -> Result<(), AnyError> {
    if let Some(doc) = self.open_docs.get(specifier) {
      doc.update_navigation_tree_if_version(navigation_tree, script_version)
    } else {
      let mut file_system_docs = self.file_system_docs.lock();
      if let Some(doc) = file_system_docs.docs.get_mut(specifier) {
        doc.update_navigation_tree_if_version(navigation_tree, script_version);
      } else {
        return Err(custom_error(
          "NotFound",
          format!("Specifier not found {specifier}"),
        ));
      }
    }
    Ok(())
  }

  pub fn update_config(&mut self, options: UpdateDocumentConfigOptions) {
    fn calculate_resolver_config_hash(
      enabled_paths: &[PathBuf],
      document_preload_limit: usize,
      maybe_import_map: Option<&import_map::ImportMap>,
      maybe_jsx_config: Option<&JsxImportSourceConfig>,
      maybe_vendor_dir: Option<bool>,
      maybe_package_json_deps: Option<&PackageJsonDeps>,
      maybe_unstable_flags: Option<&Vec<String>>,
    ) -> u64 {
      let mut hasher = FastInsecureHasher::default();
      hasher.write_hashable(document_preload_limit);
      hasher.write_hashable(&{
        // ensure these are sorted so the hashing is deterministic
        let mut enabled_paths = enabled_paths.to_vec();
        enabled_paths.sort_unstable();
        enabled_paths
      });
      if let Some(import_map) = maybe_import_map {
        hasher.write_str(&import_map.to_json());
        hasher.write_str(import_map.base_url().as_str());
      }
      hasher.write_hashable(maybe_vendor_dir);
      hasher.write_hashable(maybe_jsx_config);
      hasher.write_hashable(maybe_unstable_flags);
      if let Some(package_json_deps) = &maybe_package_json_deps {
        // We need to ensure the hashing is deterministic so explicitly type
        // this in order to catch if the type of package_json_deps ever changes
        // from a deterministic IndexMap to something else.
        let package_json_deps: &IndexMap<_, _> = *package_json_deps;
        for (key, value) in package_json_deps {
          hasher.write_hashable(key);
          match value {
            Ok(value) => {
              hasher.write_hashable(value);
            }
            Err(err) => {
              hasher.write_str(&err.to_string());
            }
          }
        }
      }

      hasher.finish()
    }

    let maybe_package_json_deps =
      options.maybe_package_json.map(|package_json| {
        package_json::get_local_package_json_version_reqs(package_json)
      });
    let maybe_jsx_config = options
      .maybe_config_file
      .and_then(|cf| cf.to_maybe_jsx_import_source_config().ok().flatten());
    let new_resolver_config_hash = calculate_resolver_config_hash(
      &options.enabled_paths,
      options.document_preload_limit,
      options.maybe_import_map.as_deref(),
      maybe_jsx_config.as_ref(),
      options.maybe_config_file.and_then(|c| c.vendor_dir_flag()),
      maybe_package_json_deps.as_ref(),
      options.maybe_config_file.map(|c| &c.json.unstable),
    );
    let deps_provider =
      Arc::new(PackageJsonDepsProvider::new(maybe_package_json_deps));
    let fs = Arc::new(RealFs);
    self.resolver = Arc::new(CliGraphResolver::new(CliGraphResolverOptions {
      fs: fs.clone(),
      node_resolver: options.node_resolver,
      npm_resolver: options.npm_resolver,
      cjs_resolutions: None, // only used for runtime
      package_json_deps_provider: deps_provider,
      maybe_jsx_import_source_config: maybe_jsx_config,
      maybe_import_map: options.maybe_import_map,
      maybe_vendor_dir: options
        .maybe_config_file
        .and_then(|c| c.vendor_dir_path())
        .as_ref(),
      bare_node_builtins_enabled: options
        .maybe_config_file
        .map(|config| config.has_unstable("bare-node-builtins"))
        .unwrap_or(false),
      // Don't set this for the LSP because instead we'll use the OpenDocumentsLoader
      // because it's much easier and we get diagnostics/quick fixes about a redirected
      // specifier for free.
      sloppy_imports_resolver: None,
    }));
    self.redirect_resolver =
      Arc::new(RedirectResolver::new(self.cache.clone()));
    self.imports = Arc::new(
      if let Some(Ok(imports)) =
        options.maybe_config_file.map(|cf| cf.to_maybe_imports())
      {
        imports
          .into_iter()
          .map(|(referrer, imports)| {
            let graph_import = GraphImport::new(
              &referrer,
              imports,
              Some(self.get_resolver()),
              Some(self.get_npm_resolver()),
            );
            (referrer, graph_import)
          })
          .collect()
      } else {
        IndexMap::new()
      },
    );
    self.unstable_sloppy_imports = options
      .maybe_config_file
      .map(|c| c.has_unstable("sloppy-imports"))
      .unwrap_or(false);

    // only refresh the dependencies if the underlying configuration has changed
    if self.resolver_config_hash != new_resolver_config_hash {
      self.refresh_dependencies(
        options.enabled_paths,
        options.disabled_paths,
        options.document_preload_limit,
      );
      self.resolver_config_hash = new_resolver_config_hash;

      self.dirty = true;
      self.calculate_dependents_if_dirty();
    }
  }

  fn refresh_dependencies(
    &mut self,
    enabled_paths: Vec<PathBuf>,
    disabled_paths: Vec<PathBuf>,
    document_preload_limit: usize,
  ) {
    let resolver = self.resolver.as_graph_resolver();
    let npm_resolver = self.resolver.as_graph_npm_resolver();
    for doc in self.open_docs.values_mut() {
      if let Some(new_doc) = doc.maybe_with_new_resolver(resolver, npm_resolver)
      {
        *doc = new_doc;
      }
    }

    // update the file system documents
    let mut fs_docs = self.file_system_docs.lock();
    if document_preload_limit > 0 {
      let mut not_found_docs =
        fs_docs.docs.keys().cloned().collect::<HashSet<_>>();
      let open_docs = &mut self.open_docs;

      log::debug!("Preloading documents from enabled urls...");
      let mut finder =
        PreloadDocumentFinder::new(PreloadDocumentFinderOptions {
          enabled_paths,
          disabled_paths,
          limit: document_preload_limit,
        });
      for specifier in finder.by_ref() {
        // mark this document as having been found
        not_found_docs.remove(&specifier);

        if !open_docs.contains_key(&specifier)
          && !fs_docs.docs.contains_key(&specifier)
        {
          fs_docs.refresh_document(
            &self.cache,
            resolver,
            &specifier,
            npm_resolver,
          );
        } else {
          // update the existing entry to have the new resolver
          if let Some(doc) = fs_docs.docs.get_mut(&specifier) {
            if let Some(new_doc) =
              doc.maybe_with_new_resolver(resolver, npm_resolver)
            {
              *doc = new_doc;
            }
          }
        }
      }

      if finder.hit_limit() {
        lsp_warn!(
            concat!(
              "Hit the language server document preload limit of {} file system entries. ",
              "You may want to use the \"deno.enablePaths\" configuration setting to only have Deno ",
              "partially enable a workspace or increase the limit via \"deno.documentPreloadLimit\". ",
              "In cases where Deno ends up using too much memory, you may want to lower the limit."
            ),
            document_preload_limit,
          );

        // since we hit the limit, just update everything to use the new resolver
        for uri in not_found_docs {
          if let Some(doc) = fs_docs.docs.get_mut(&uri) {
            if let Some(new_doc) =
              doc.maybe_with_new_resolver(resolver, npm_resolver)
            {
              *doc = new_doc;
            }
          }
        }
      } else {
        // clean up and remove any documents that weren't found
        for uri in not_found_docs {
          fs_docs.docs.remove(&uri);
        }
      }
    } else {
      // This log statement is used in the tests to ensure preloading doesn't
      // happen, which is not useful in the repl and could be very expensive
      // if the repl is launched from a directory with a lot of descendants.
      log::debug!("Skipping document preload.");

      // just update to use the new resolver
      for doc in fs_docs.docs.values_mut() {
        if let Some(new_doc) =
          doc.maybe_with_new_resolver(resolver, npm_resolver)
        {
          *doc = new_doc;
        }
      }
    }

    fs_docs.dirty = true;
  }

  /// Iterate through the documents, building a map where the key is a unique
  /// document and the value is a set of specifiers that depend on that
  /// document.
  fn calculate_dependents_if_dirty(&mut self) {
    #[derive(Default)]
    struct DocAnalyzer {
      dependents_map: HashMap<ModuleSpecifier, HashSet<ModuleSpecifier>>,
      analyzed_specifiers: HashSet<ModuleSpecifier>,
      pending_specifiers: VecDeque<ModuleSpecifier>,
      npm_reqs: HashSet<PackageReq>,
      has_node_builtin_specifier: bool,
    }

    impl DocAnalyzer {
      fn add(&mut self, dep: &ModuleSpecifier, specifier: &ModuleSpecifier) {
        if !self.analyzed_specifiers.contains(dep) {
          self.analyzed_specifiers.insert(dep.clone());
          // perf: ensure this is not added to unless this specifier has never
          // been analyzed in order to not cause an extra file system lookup
          self.pending_specifiers.push_back(dep.clone());
          if let Ok(reference) = NpmPackageReqReference::from_specifier(dep) {
            self.npm_reqs.insert(reference.into_inner().req);
          }
        }

        self
          .dependents_map
          .entry(dep.clone())
          .or_default()
          .insert(specifier.clone());
      }

      fn analyze_doc(&mut self, specifier: &ModuleSpecifier, doc: &Document) {
        self.analyzed_specifiers.insert(specifier.clone());
        for dependency in doc.dependencies().values() {
          if let Some(dep) = dependency.get_code() {
            if !self.has_node_builtin_specifier && dep.scheme() == "node" {
              self.has_node_builtin_specifier = true;
            }
            self.add(dep, specifier);
          }
          if let Some(dep) = dependency.get_type() {
            self.add(dep, specifier);
          }
        }
        if let Some(dep) = doc.maybe_types_dependency().maybe_specifier() {
          self.add(dep, specifier);
        }
      }
    }

    let mut file_system_docs = self.file_system_docs.lock();
    if !file_system_docs.dirty && !self.dirty {
      return;
    }

    let mut doc_analyzer = DocAnalyzer::default();
    // favor documents that are open in case a document exists in both collections
    let documents = file_system_docs.docs.iter().chain(self.open_docs.iter());
    for (specifier, doc) in documents {
      doc_analyzer.analyze_doc(specifier, doc);
    }

    let resolver = self.get_resolver();
    let npm_resolver = self.get_npm_resolver();
    while let Some(specifier) = doc_analyzer.pending_specifiers.pop_front() {
      if let Some(doc) = self.open_docs.get(&specifier) {
        doc_analyzer.analyze_doc(&specifier, doc);
      } else if let Some(doc) =
        file_system_docs.get(&self.cache, resolver, &specifier, npm_resolver)
      {
        doc_analyzer.analyze_doc(&specifier, &doc);
      }
    }

    let mut npm_reqs = doc_analyzer.npm_reqs;
    // Ensure a @types/node package exists when any module uses a node: specifier.
    // Unlike on the command line, here we just add @types/node to the npm package
    // requirements since this won't end up in the lockfile.
    self.has_injected_types_node_package = doc_analyzer
      .has_node_builtin_specifier
      && !npm_reqs.iter().any(|r| r.name == "@types/node");
    if self.has_injected_types_node_package {
      npm_reqs.insert(PackageReq::from_str("@types/node").unwrap());
    }

    self.dependents_map = Arc::new(doc_analyzer.dependents_map);
    self.npm_specifier_reqs = Arc::new({
      let mut reqs = npm_reqs.into_iter().collect::<Vec<_>>();
      reqs.sort();
      reqs
    });
    self.dirty = false;
    file_system_docs.dirty = false;
  }

  fn get_resolver(&self) -> &dyn deno_graph::source::Resolver {
    self.resolver.as_graph_resolver()
  }

  fn get_npm_resolver(&self) -> &dyn deno_graph::source::NpmResolver {
    self.resolver.as_graph_npm_resolver()
  }

  fn resolve_dependency(
    &self,
    specifier: &ModuleSpecifier,
    maybe_npm: Option<&StateNpmSnapshot>,
    referrer: &ModuleSpecifier,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    if let Some(module_name) = specifier.as_str().strip_prefix("node:") {
      if deno_node::is_builtin_node_module(module_name) {
        // return itself for node: specifiers because during type checking
        // we resolve to the ambient modules in the @types/node package
        // rather than deno_std/node
        return Some((specifier.clone(), MediaType::Dts));
      }
    }

    if let Ok(npm_ref) = NpmPackageReqReference::from_specifier(specifier) {
      return node_resolve_npm_req_ref(npm_ref, maybe_npm, referrer);
    }
    let doc = self.get(specifier)?;
    let maybe_module = doc.maybe_esm_module().and_then(|r| r.as_ref().ok());
    let maybe_types_dependency = maybe_module
      .and_then(|m| m.maybe_types_dependency.as_ref().map(|d| &d.dependency));
    if let Some(specifier) =
      maybe_types_dependency.and_then(|d| d.maybe_specifier())
    {
      self.resolve_dependency(specifier, maybe_npm, referrer)
    } else {
      let media_type = doc.media_type();
      Some((doc.specifier().clone(), media_type))
    }
  }

  /// Iterate through any "imported" modules, checking to see if a dependency
  /// is available. This is used to provide "global" imports like the JSX import
  /// source.
  fn resolve_imports_dependency(&self, specifier: &str) -> Option<&Resolution> {
    for graph_imports in self.imports.values() {
      let maybe_dep = graph_imports.dependencies.get(specifier);
      if maybe_dep.is_some() {
        return maybe_dep.map(|d| &d.maybe_type);
      }
    }
    None
  }
}

fn node_resolve_npm_req_ref(
  npm_req_ref: NpmPackageReqReference,
  maybe_npm: Option<&StateNpmSnapshot>,
  referrer: &ModuleSpecifier,
) -> Option<(ModuleSpecifier, MediaType)> {
  maybe_npm.map(|npm| {
    NodeResolution::into_specifier_and_media_type(
      npm
        .npm_resolver
        .resolve_pkg_folder_from_deno_module_req(npm_req_ref.req(), referrer)
        .ok()
        .and_then(|package_folder| {
          npm
            .node_resolver
            .resolve_package_subpath_from_deno_module(
              &package_folder,
              npm_req_ref.sub_path(),
              referrer,
              NodeResolutionMode::Types,
              &PermissionsContainer::allow_all(),
            )
            .ok()
            .flatten()
        }),
    )
  })
}

/// Loader that will look at the open documents.
pub struct OpenDocumentsGraphLoader<'a> {
  pub inner_loader: &'a mut dyn deno_graph::source::Loader,
  pub open_docs: &'a HashMap<ModuleSpecifier, Document>,
  pub unstable_sloppy_imports: bool,
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
            content: doc.content(),
            specifier: doc.specifier().clone(),
            maybe_headers: None,
          })))
          .boxed_local(),
        );
      }
    }
    None
  }

  fn resolve_unstable_sloppy_import<'b>(
    &self,
    specifier: &'b ModuleSpecifier,
  ) -> SloppyImportsResolution<'b> {
    SloppyImportsResolver::resolve_with_stat_sync(specifier, |path| {
      if let Ok(specifier) = ModuleSpecifier::from_file_path(path) {
        if self.open_docs.contains_key(&specifier) {
          return Some(SloppyImportsFsEntry::File);
        }
      }
      path.metadata().ok().and_then(|m| {
        if m.is_file() {
          Some(SloppyImportsFsEntry::File)
        } else if m.is_dir() {
          Some(SloppyImportsFsEntry::Dir)
        } else {
          None
        }
      })
    })
  }
}

impl<'a> deno_graph::source::Loader for OpenDocumentsGraphLoader<'a> {
  fn registry_url(&self) -> &Url {
    self.inner_loader.registry_url()
  }

  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    is_dynamic: bool,
    cache_setting: deno_graph::source::CacheSetting,
  ) -> deno_graph::source::LoadFuture {
    let specifier = if self.unstable_sloppy_imports {
      self
        .resolve_unstable_sloppy_import(specifier)
        .into_specifier()
    } else {
      Cow::Borrowed(specifier)
    };

    match self.load_from_docs(&specifier) {
      Some(fut) => fut,
      None => self
        .inner_loader
        .load(&specifier, is_dynamic, cache_setting),
    }
  }

  fn cache_module_info(
    &mut self,
    specifier: &deno_ast::ModuleSpecifier,
    source: &str,
    module_info: &deno_graph::ModuleInfo,
  ) {
    self
      .inner_loader
      .cache_module_info(specifier, source, module_info)
  }
}

fn parse_and_analyze_module(
  specifier: &ModuleSpecifier,
  text_info: SourceTextInfo,
  maybe_headers: Option<&HashMap<String, String>>,
  resolver: &dyn deno_graph::source::Resolver,
  npm_resolver: &dyn deno_graph::source::NpmResolver,
) -> (Option<ParsedSourceResult>, Option<ModuleResult>) {
  let parsed_source_result = parse_source(specifier, text_info, maybe_headers);
  let module_result = analyze_module(
    specifier,
    &parsed_source_result,
    maybe_headers,
    resolver,
    npm_resolver,
  );
  (Some(parsed_source_result), Some(module_result))
}

fn parse_source(
  specifier: &ModuleSpecifier,
  text_info: SourceTextInfo,
  maybe_headers: Option<&HashMap<String, String>>,
) -> ParsedSourceResult {
  deno_ast::parse_module(deno_ast::ParseParams {
    specifier: specifier.to_string(),
    text_info,
    media_type: MediaType::from_specifier_and_headers(specifier, maybe_headers),
    capture_tokens: true,
    scope_analysis: true,
    maybe_syntax: None,
  })
}

fn analyze_module(
  specifier: &ModuleSpecifier,
  parsed_source_result: &ParsedSourceResult,
  maybe_headers: Option<&HashMap<String, String>>,
  resolver: &dyn deno_graph::source::Resolver,
  npm_resolver: &dyn deno_graph::source::NpmResolver,
) -> ModuleResult {
  match parsed_source_result {
    Ok(parsed_source) => Ok(deno_graph::parse_module_from_ast(
      deno_graph::ParseModuleFromAstOptions {
        graph_kind: deno_graph::GraphKind::All,
        specifier,
        maybe_headers,
        parsed_source,
        // use a null file system because there's no need to bother resolving
        // dynamic imports like import(`./dir/${something}`) in the LSP
        file_system: &deno_graph::source::NullFileSystem,
        maybe_resolver: Some(resolver),
        maybe_npm_resolver: Some(npm_resolver),
      },
    )),
    Err(err) => Err(deno_graph::ModuleGraphError::ModuleError(
      deno_graph::ModuleError::ParseErr(specifier.clone(), err.clone()),
    )),
  }
}

enum PendingEntry {
  /// File specified as a root url.
  SpecifiedRootFile(PathBuf),
  /// Directory that is queued to read.
  Dir(PathBuf),
  /// The current directory being read.
  ReadDir(Box<ReadDir>),
}

struct PreloadDocumentFinderOptions {
  enabled_paths: Vec<PathBuf>,
  disabled_paths: Vec<PathBuf>,
  limit: usize,
}

/// Iterator that finds documents that can be preloaded into
/// the LSP on startup.
struct PreloadDocumentFinder {
  limit: usize,
  entry_count: usize,
  pending_entries: VecDeque<PendingEntry>,
  disabled_globs: glob::GlobSet,
  disabled_paths: HashSet<PathBuf>,
}

impl PreloadDocumentFinder {
  pub fn new(options: PreloadDocumentFinderOptions) -> Self {
    fn paths_into_globs_and_paths(
      input_paths: Vec<PathBuf>,
    ) -> (glob::GlobSet, HashSet<PathBuf>) {
      let mut globs = Vec::with_capacity(input_paths.len());
      let mut paths = HashSet::with_capacity(input_paths.len());
      for path in input_paths {
        if let Ok(Some(glob)) =
          glob::GlobPattern::new_if_pattern(&path.to_string_lossy())
        {
          globs.push(glob);
        } else {
          paths.insert(path);
        }
      }
      (glob::GlobSet::new(globs), paths)
    }

    fn is_allowed_root_dir(dir_path: &Path) -> bool {
      if dir_path.parent().is_none() {
        // never search the root directory of a drive
        return false;
      }
      true
    }

    let (disabled_globs, disabled_paths) =
      paths_into_globs_and_paths(options.disabled_paths);
    let mut finder = PreloadDocumentFinder {
      limit: options.limit,
      entry_count: 0,
      pending_entries: Default::default(),
      disabled_globs,
      disabled_paths,
    };

    // initialize the finder with the initial paths
    let mut dirs = Vec::with_capacity(options.enabled_paths.len());
    for path in options.enabled_paths {
      if !finder.disabled_paths.contains(&path)
        && !finder.disabled_globs.matches_path(&path)
      {
        if path.is_dir() {
          if is_allowed_root_dir(&path) {
            dirs.push(path);
          }
        } else {
          finder
            .pending_entries
            .push_back(PendingEntry::SpecifiedRootFile(path));
        }
      }
    }
    for dir in sort_and_remove_non_leaf_dirs(dirs) {
      finder.pending_entries.push_back(PendingEntry::Dir(dir));
    }
    finder
  }

  pub fn hit_limit(&self) -> bool {
    self.entry_count >= self.limit
  }

  fn get_valid_specifier(path: &Path) -> Option<ModuleSpecifier> {
    fn is_allowed_media_type(media_type: MediaType) -> bool {
      match media_type {
        MediaType::JavaScript
        | MediaType::Jsx
        | MediaType::Mjs
        | MediaType::Cjs
        | MediaType::TypeScript
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Dts
        | MediaType::Dmts
        | MediaType::Dcts
        | MediaType::Tsx => true,
        MediaType::Json // ignore because json never depends on other files
        | MediaType::Wasm
        | MediaType::SourceMap
        | MediaType::TsBuildInfo
        | MediaType::Unknown => false,
      }
    }

    let media_type = MediaType::from_path(path);
    if is_allowed_media_type(media_type) {
      if let Ok(specifier) = ModuleSpecifier::from_file_path(path) {
        return Some(specifier);
      }
    }
    None
  }
}

impl Iterator for PreloadDocumentFinder {
  type Item = ModuleSpecifier;

  fn next(&mut self) -> Option<Self::Item> {
    fn is_discoverable_dir(dir_path: &Path) -> bool {
      if let Some(dir_name) = dir_path.file_name() {
        let dir_name = dir_name.to_string_lossy().to_lowercase();
        // We ignore these directories by default because there is a
        // high likelihood they aren't relevant. Someone can opt-into
        // them by specifying one of them as an enabled path.
        if matches!(dir_name.as_str(), "node_modules" | ".git") {
          return false;
        }

        // ignore cargo target directories for anyone using Deno with Rust
        if dir_name == "target"
          && dir_path
            .parent()
            .map(|p| p.join("Cargo.toml").exists())
            .unwrap_or(false)
        {
          return false;
        }

        true
      } else {
        false
      }
    }

    fn is_discoverable_file(file_path: &Path) -> bool {
      // Don't auto-discover minified files as they are likely to be very large
      // and likely not to have dependencies on code outside them that would
      // be useful in the LSP
      if let Some(file_name) = file_path.file_name() {
        let file_name = file_name.to_string_lossy().to_lowercase();
        !file_name.as_str().contains(".min.")
      } else {
        false
      }
    }

    while let Some(entry) = self.pending_entries.pop_front() {
      match entry {
        PendingEntry::SpecifiedRootFile(file) => {
          // since it was a file that was specified as a root url, only
          // verify that it's valid
          if let Some(specifier) = Self::get_valid_specifier(&file) {
            return Some(specifier);
          }
        }
        PendingEntry::Dir(dir_path) => {
          if let Ok(read_dir) = fs::read_dir(&dir_path) {
            self
              .pending_entries
              .push_back(PendingEntry::ReadDir(Box::new(read_dir)));
          }
        }
        PendingEntry::ReadDir(mut entries) => {
          while let Some(entry) = entries.next() {
            self.entry_count += 1;

            if self.hit_limit() {
              self.pending_entries.clear(); // stop searching
              return None;
            }

            if let Ok(entry) = entry {
              let path = entry.path();
              if let Ok(file_type) = entry.file_type() {
                if !self.disabled_paths.contains(&path)
                  && !self.disabled_globs.matches_path(&path)
                {
                  if file_type.is_dir() && is_discoverable_dir(&path) {
                    self
                      .pending_entries
                      .push_back(PendingEntry::Dir(path.to_path_buf()));
                  } else if file_type.is_file() && is_discoverable_file(&path) {
                    if let Some(specifier) = Self::get_valid_specifier(&path) {
                      // restore the next entries for next time
                      self
                        .pending_entries
                        .push_front(PendingEntry::ReadDir(entries));
                      return Some(specifier);
                    }
                  }
                }
              }
            }
          }
        }
      }
    }

    None
  }
}

/// Removes any directories that are a descendant of another directory in the collection.
fn sort_and_remove_non_leaf_dirs(mut dirs: Vec<PathBuf>) -> Vec<PathBuf> {
  if dirs.is_empty() {
    return dirs;
  }

  dirs.sort();
  if !dirs.is_empty() {
    for i in (0..dirs.len() - 1).rev() {
      let prev = &dirs[i + 1];
      if prev.starts_with(&dirs[i]) {
        dirs.remove(i + 1);
      }
    }
  }

  dirs
}

#[cfg(test)]
mod tests {
  use crate::cache::GlobalHttpCache;
  use crate::cache::RealDenoCacheEnv;

  use super::*;
  use import_map::ImportMap;
  use pretty_assertions::assert_eq;
  use test_util::PathRef;
  use test_util::TempDir;

  fn setup(temp_dir: &TempDir) -> (Documents, PathRef) {
    let location = temp_dir.path().join("deps");
    let cache = Arc::new(GlobalHttpCache::new(
      location.to_path_buf(),
      RealDenoCacheEnv,
    ));
    let documents = Documents::new(cache);
    (documents, location)
  }

  #[test]
  fn test_documents_open() {
    let temp_dir = TempDir::new();
    let (mut documents, _) = setup(&temp_dir);
    let specifier = ModuleSpecifier::parse("file:///a.ts").unwrap();
    let content = r#"import * as b from "./b.ts";
console.log(b);
"#;
    let document = documents.open(
      specifier,
      1,
      "javascript".parse().unwrap(),
      content.into(),
    );
    assert!(document.is_open());
    assert!(document.is_diagnosable());
  }

  #[test]
  fn test_documents_change() {
    let temp_dir = TempDir::new();
    let (mut documents, _) = setup(&temp_dir);
    let specifier = ModuleSpecifier::parse("file:///a.ts").unwrap();
    let content = r#"import * as b from "./b.ts";
console.log(b);
"#;
    documents.open(
      specifier.clone(),
      1,
      "javascript".parse().unwrap(),
      content.into(),
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
      &*documents.get(&specifier).unwrap().content(),
      r#"import * as b from "./b.ts";
console.log(b, "hello deno");
"#
    );
  }

  #[test]
  fn test_documents_ensure_no_duplicates() {
    // it should never happen that a user of this API causes this to happen,
    // but we'll guard against it anyway
    let temp_dir = TempDir::new();
    let (mut documents, documents_path) = setup(&temp_dir);
    let file_path = documents_path.join("file.ts");
    let file_specifier = ModuleSpecifier::from_file_path(&file_path).unwrap();
    documents_path.create_dir_all();
    file_path.write("");

    // open the document
    documents.open(
      file_specifier.clone(),
      1,
      LanguageId::TypeScript,
      "".into(),
    );

    // make a clone of the document store and close the document in that one
    let mut documents2 = documents.clone();
    documents2.close(&file_specifier).unwrap();

    // At this point the document will be in both documents and the shared file system documents.
    // Now make sure that the original documents doesn't return both copies
    assert_eq!(documents.documents(DocumentsFilter::All).len(), 1);
  }

  #[test]
  fn test_documents_refresh_dependencies_config_change() {
    // it should never happen that a user of this API causes this to happen,
    // but we'll guard against it anyway
    let temp_dir = TempDir::new();
    let (mut documents, documents_path) = setup(&temp_dir);
    fs::create_dir_all(&documents_path).unwrap();

    let file1_path = documents_path.join("file1.ts");
    let file1_specifier = ModuleSpecifier::from_file_path(&file1_path).unwrap();
    fs::write(&file1_path, "").unwrap();

    let file2_path = documents_path.join("file2.ts");
    let file2_specifier = ModuleSpecifier::from_file_path(&file2_path).unwrap();
    fs::write(&file2_path, "").unwrap();

    let file3_path = documents_path.join("file3.ts");
    let file3_specifier = ModuleSpecifier::from_file_path(&file3_path).unwrap();
    fs::write(&file3_path, "").unwrap();

    // set the initial import map and point to file 2
    {
      let mut import_map = ImportMap::new(
        ModuleSpecifier::from_file_path(documents_path.join("import_map.json"))
          .unwrap(),
      );
      import_map
        .imports_mut()
        .append("test".to_string(), "./file2.ts".to_string())
        .unwrap();

      documents.update_config(UpdateDocumentConfigOptions {
        enabled_paths: vec![],
        disabled_paths: vec![],
        document_preload_limit: 1_000,
        maybe_import_map: Some(Arc::new(import_map)),
        maybe_config_file: None,
        maybe_package_json: None,
        node_resolver: None,
        npm_resolver: None,
      });

      // open the document
      let document = documents.open(
        file1_specifier.clone(),
        1,
        LanguageId::TypeScript,
        "import {} from 'test';".into(),
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
      let mut import_map = ImportMap::new(
        ModuleSpecifier::from_file_path(documents_path.join("import_map.json"))
          .unwrap(),
      );
      import_map
        .imports_mut()
        .append("test".to_string(), "./file3.ts".to_string())
        .unwrap();

      documents.update_config(UpdateDocumentConfigOptions {
        enabled_paths: vec![],
        disabled_paths: vec![],
        document_preload_limit: 1_000,
        maybe_import_map: Some(Arc::new(import_map)),
        maybe_config_file: None,
        maybe_package_json: None,
        node_resolver: None,
        npm_resolver: None,
      });

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

  #[test]
  pub fn test_pre_load_document_finder() {
    let temp_dir = TempDir::new();
    temp_dir.create_dir_all("root1/node_modules/");
    temp_dir.write("root1/node_modules/mod.ts", ""); // no, node_modules

    temp_dir.create_dir_all("root1/sub_dir");
    temp_dir.create_dir_all("root1/target");
    temp_dir.create_dir_all("root1/node_modules");
    temp_dir.create_dir_all("root1/.git");
    temp_dir.create_dir_all("root1/file.ts"); // no, directory
    temp_dir.write("root1/mod1.ts", ""); // yes
    temp_dir.write("root1/mod2.js", ""); // yes
    temp_dir.write("root1/mod3.tsx", ""); // yes
    temp_dir.write("root1/mod4.d.ts", ""); // yes
    temp_dir.write("root1/mod5.jsx", ""); // yes
    temp_dir.write("root1/mod6.mjs", ""); // yes
    temp_dir.write("root1/mod7.mts", ""); // yes
    temp_dir.write("root1/mod8.d.mts", ""); // yes
    temp_dir.write("root1/other.json", ""); // no, json
    temp_dir.write("root1/other.txt", ""); // no, text file
    temp_dir.write("root1/other.wasm", ""); // no, don't load wasm
    temp_dir.write("root1/Cargo.toml", ""); // no
    temp_dir.write("root1/sub_dir/mod.ts", ""); // yes
    temp_dir.write("root1/sub_dir/data.min.ts", ""); // no, minified file
    temp_dir.write("root1/.git/main.ts", ""); // no, .git folder
    temp_dir.write("root1/node_modules/main.ts", ""); // no, because it's in a node_modules folder
    temp_dir.write("root1/target/main.ts", ""); // no, because there is a Cargo.toml in the root directory

    temp_dir.create_dir_all("root2/folder");
    temp_dir.create_dir_all("root2/sub_folder");
    temp_dir.write("root2/file1.ts", ""); // yes, provided
    temp_dir.write("root2/file2.ts", ""); // no, not provided
    temp_dir.write("root2/main.min.ts", ""); // yes, provided
    temp_dir.write("root2/folder/main.ts", ""); // yes, provided
    temp_dir.write("root2/sub_folder/a.js", ""); // no, not provided
    temp_dir.write("root2/sub_folder/b.ts", ""); // no, not provided
    temp_dir.write("root2/sub_folder/c.js", ""); // no, not provided

    temp_dir.create_dir_all("root3/");
    temp_dir.write("root3/mod.ts", ""); // no, not provided

    let mut urls = PreloadDocumentFinder::new(PreloadDocumentFinderOptions {
      enabled_paths: vec![
        temp_dir.path().to_path_buf().join("root1"),
        temp_dir.path().to_path_buf().join("root2").join("file1.ts"),
        temp_dir
          .path()
          .to_path_buf()
          .join("root2")
          .join("main.min.ts"),
        temp_dir.path().to_path_buf().join("root2").join("folder"),
      ],
      disabled_paths: Vec::new(),
      limit: 1_000,
    })
    .collect::<Vec<_>>();

    // Ideally we would test for order here, which should be BFS, but
    // different file systems have different directory iteration
    // so we sort the results
    urls.sort();

    assert_eq!(
      urls,
      vec![
        temp_dir.uri().join("root1/mod1.ts").unwrap(),
        temp_dir.uri().join("root1/mod2.js").unwrap(),
        temp_dir.uri().join("root1/mod3.tsx").unwrap(),
        temp_dir.uri().join("root1/mod4.d.ts").unwrap(),
        temp_dir.uri().join("root1/mod5.jsx").unwrap(),
        temp_dir.uri().join("root1/mod6.mjs").unwrap(),
        temp_dir.uri().join("root1/mod7.mts").unwrap(),
        temp_dir.uri().join("root1/mod8.d.mts").unwrap(),
        temp_dir.uri().join("root1/sub_dir/mod.ts").unwrap(),
        temp_dir.uri().join("root2/file1.ts").unwrap(),
        temp_dir.uri().join("root2/folder/main.ts").unwrap(),
        temp_dir.uri().join("root2/main.min.ts").unwrap(),
      ]
    );

    // now try iterating with a low limit
    let urls = PreloadDocumentFinder::new(PreloadDocumentFinderOptions {
      enabled_paths: vec![temp_dir.path().to_path_buf()],
      disabled_paths: Vec::new(),
      limit: 10, // entries and not results
    })
    .collect::<Vec<_>>();

    // since different file system have different iteration
    // order, the number here may vary, so just assert it's below
    // a certain amount
    assert!(urls.len() < 5, "Actual length: {}", urls.len());

    // now try with certain directories and files disabled
    let mut urls = PreloadDocumentFinder::new(PreloadDocumentFinderOptions {
      enabled_paths: vec![temp_dir.path().to_path_buf()],
      disabled_paths: vec![
        temp_dir.path().to_path_buf().join("root1"),
        temp_dir.path().to_path_buf().join("root2").join("file1.ts"),
        temp_dir.path().to_path_buf().join("**/*.js"), // ignore js files
      ],
      limit: 1_000,
    })
    .collect::<Vec<_>>();
    urls.sort();
    assert_eq!(
      urls,
      vec![
        temp_dir.uri().join("root2/file2.ts").unwrap(),
        temp_dir.uri().join("root2/folder/main.ts").unwrap(),
        temp_dir.uri().join("root2/sub_folder/b.ts").unwrap(), // won't have the javascript files
        temp_dir.uri().join("root3/mod.ts").unwrap(),
      ]
    );
  }

  #[test]
  pub fn test_pre_load_document_finder_disallowed_dirs() {
    if cfg!(windows) {
      let paths = PreloadDocumentFinder::new(PreloadDocumentFinderOptions {
        enabled_paths: vec![PathBuf::from("C:\\")],
        disabled_paths: Vec::new(),
        limit: 1_000,
      })
      .collect::<Vec<_>>();
      assert_eq!(paths, vec![]);
    } else {
      let paths = PreloadDocumentFinder::new(PreloadDocumentFinderOptions {
        enabled_paths: vec![PathBuf::from("/")],
        disabled_paths: Vec::new(),
        limit: 1_000,
      })
      .collect::<Vec<_>>();
      assert_eq!(paths, vec![]);
    }
  }

  #[test]
  fn test_sort_and_remove_non_leaf_dirs() {
    fn run_test(paths: Vec<&str>, expected_output: Vec<&str>) {
      let paths = sort_and_remove_non_leaf_dirs(
        paths.into_iter().map(PathBuf::from).collect(),
      );
      let dirs: Vec<_> =
        paths.iter().map(|dir| dir.to_string_lossy()).collect();
      assert_eq!(dirs, expected_output);
    }

    run_test(
      vec![
        "/test/asdf/test/asdf/",
        "/test/asdf/test/asdf/test.ts",
        "/test/asdf/",
        "/test/asdf/",
        "/testing/456/893/",
        "/testing/456/893/test/",
      ],
      vec!["/test/asdf/", "/testing/456/893/"],
    );
    run_test(vec![], vec![]);
  }
}
