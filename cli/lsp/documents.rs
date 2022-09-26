// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::cache::calculate_fs_version;
use super::text::LineIndex;
use super::tsc;
use super::tsc::AssetDocument;

use crate::args::ConfigFile;
use crate::file_fetcher::get_source_from_bytes;
use crate::file_fetcher::map_content_type;
use crate::file_fetcher::SUPPORTED_SCHEMES;
use crate::fs_util::specifier_to_file_path;
use crate::http_cache;
use crate::http_cache::HttpCache;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;
use crate::text_encoding;

use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url;
use deno_core::ModuleSpecifier;
use deno_graph::GraphImport;
use deno_graph::Resolved;
use once_cell::sync::Lazy;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
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
  .iter()
  .cloned()
  .collect()
});

static JSX_HEADERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
  ([("content-type".to_string(), "text/jsx".to_string())])
    .iter()
    .cloned()
    .collect()
});

static TS_HEADERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
  ([(
    "content-type".to_string(),
    "application/typescript".to_string(),
  )])
  .iter()
  .cloned()
  .collect()
});

static TSX_HEADERS: Lazy<HashMap<String, String>> = Lazy::new(|| {
  ([("content-type".to_string(), "text/tsx".to_string())])
    .iter()
    .cloned()
    .collect()
});

#[derive(Debug, Clone, PartialEq, Eq)]
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

type MaybeModuleResult =
  Option<Result<deno_graph::Module, deno_graph::ModuleGraphError>>;
type MaybeParsedSourceResult =
  Option<Result<ParsedSource, deno_ast::Diagnostic>>;

#[derive(Debug, Clone)]
struct DocumentInner {
  /// contains the last-known-good set of dependencies from parsing the module
  dependencies: Arc<BTreeMap<String, deno_graph::Dependency>>,
  fs_version: String,
  line_index: Arc<LineIndex>,
  maybe_language_id: Option<LanguageId>,
  maybe_lsp_version: Option<i32>,
  maybe_module: MaybeModuleResult,
  maybe_navigation_tree: Option<Arc<tsc::NavigationTree>>,
  maybe_parsed_source: MaybeParsedSourceResult,
  specifier: ModuleSpecifier,
  text_info: SourceTextInfo,
}

#[derive(Debug, Clone)]
pub struct Document(Arc<DocumentInner>);

impl Document {
  fn new(
    specifier: ModuleSpecifier,
    fs_version: String,
    maybe_headers: Option<&HashMap<String, String>>,
    content: Arc<str>,
    maybe_resolver: Option<&dyn deno_graph::source::Resolver>,
  ) -> Self {
    // we only ever do `Document::new` on on disk resources that are supposed to
    // be diagnosable, unlike `Document::open`, so it is safe to unconditionally
    // parse the module.
    let (maybe_module, maybe_parsed_source) = lsp_deno_graph_analyze(
      &specifier,
      content.clone(),
      maybe_headers,
      maybe_resolver,
    );
    let dependencies = if let Some(Ok(module)) = &maybe_module {
      Arc::new(module.dependencies.clone())
    } else {
      Arc::new(BTreeMap::new())
    };
    // todo(dsherret): retrieve this from the parsed source if it
    let text_info = SourceTextInfo::new(content);
    let line_index = Arc::new(LineIndex::new(text_info.text_str()));
    Self(Arc::new(DocumentInner {
      dependencies,
      fs_version,
      line_index,
      maybe_language_id: None,
      maybe_lsp_version: None,
      maybe_module,
      maybe_navigation_tree: None,
      maybe_parsed_source,
      text_info,
      specifier,
    }))
  }

  fn open(
    specifier: ModuleSpecifier,
    version: i32,
    language_id: LanguageId,
    content: Arc<str>,
    maybe_resolver: Option<&dyn deno_graph::source::Resolver>,
  ) -> Self {
    let maybe_headers = language_id.as_headers();
    let (maybe_module, maybe_parsed_source) = if language_id.is_diagnosable() {
      lsp_deno_graph_analyze(
        &specifier,
        content.clone(),
        maybe_headers,
        maybe_resolver,
      )
    } else {
      (None, None)
    };
    let dependencies = if let Some(Ok(module)) = &maybe_module {
      Arc::new(module.dependencies.clone())
    } else {
      Arc::new(BTreeMap::new())
    };
    let source = SourceTextInfo::new(content);
    let line_index = Arc::new(LineIndex::new(source.text_str()));
    Self(Arc::new(DocumentInner {
      dependencies,
      fs_version: "1".to_string(),
      line_index,
      maybe_language_id: Some(language_id),
      maybe_lsp_version: Some(version),
      maybe_module,
      maybe_navigation_tree: None,
      maybe_parsed_source,
      text_info: source,
      specifier,
    }))
  }

  fn with_change(
    &self,
    version: i32,
    changes: Vec<lsp::TextDocumentContentChangeEvent>,
    maybe_resolver: Option<&dyn deno_graph::source::Resolver>,
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
    let content: Arc<str> = content.into();
    let (maybe_module, maybe_parsed_source) = if self
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
      lsp_deno_graph_analyze(
        &self.0.specifier,
        content.clone(),
        maybe_headers,
        maybe_resolver,
      )
    } else {
      (None, None)
    };
    let dependencies = if let Some(Ok(module)) = &maybe_module {
      Arc::new(module.dependencies.clone())
    } else {
      self.0.dependencies.clone()
    };
    let text_info = SourceTextInfo::new(content);
    let line_index = if index_valid == IndexValid::All {
      line_index
    } else {
      Arc::new(LineIndex::new(text_info.text_str()))
    };
    Ok(Document(Arc::new(DocumentInner {
      dependencies,
      text_info,
      line_index,
      maybe_module,
      maybe_parsed_source,
      maybe_lsp_version: Some(version),
      maybe_navigation_tree: None,
      ..(*self.0).clone()
    })))
  }

  fn with_navigation_tree(
    &self,
    navigation_tree: Arc<tsc::NavigationTree>,
  ) -> Document {
    Document(Arc::new(DocumentInner {
      maybe_navigation_tree: Some(navigation_tree),
      ..(*self.0).clone()
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
      .map_or_else(|| self.fs_version().to_string(), |v| v.to_string())
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

  pub fn maybe_types_dependency(&self) -> deno_graph::Resolved {
    let module_result = match self.0.maybe_module.as_ref() {
      Some(module_result) => module_result,
      _ => return deno_graph::Resolved::None,
    };
    let module = match module_result.as_ref() {
      Ok(module) => module,
      Err(_) => return deno_graph::Resolved::None,
    };
    if let Some((_, maybe_dep)) = module.maybe_types_dependency.as_ref() {
      maybe_dep.clone()
    } else {
      deno_graph::Resolved::None
    }
  }

  pub fn media_type(&self) -> MediaType {
    if let Some(Ok(module)) = &self.0.maybe_module {
      module.media_type
    } else {
      MediaType::from(&self.0.specifier)
    }
  }

  /// Returns the current language server client version if any.
  pub fn maybe_lsp_version(&self) -> Option<i32> {
    self.0.maybe_lsp_version
  }

  fn maybe_module(
    &self,
  ) -> Option<&Result<deno_graph::Module, deno_graph::ModuleGraphError>> {
    self.0.maybe_module.as_ref()
  }

  pub fn maybe_parsed_source(
    &self,
  ) -> Option<Result<deno_ast::ParsedSource, deno_ast::Diagnostic>> {
    self.0.maybe_parsed_source.clone()
  }

  pub fn maybe_navigation_tree(&self) -> Option<Arc<tsc::NavigationTree>> {
    self.0.maybe_navigation_tree.clone()
  }

  pub fn dependencies(&self) -> Vec<(String, deno_graph::Dependency)> {
    self
      .0
      .dependencies
      .iter()
      .map(|(s, d)| (s.clone(), d.clone()))
      .collect()
  }

  /// If the supplied position is within a dependency range, return the resolved
  /// string specifier for the dependency, the resolved dependency and the range
  /// in the source document of the specifier.
  pub fn get_maybe_dependency(
    &self,
    position: &lsp::Position,
  ) -> Option<(String, deno_graph::Dependency, deno_graph::Range)> {
    let module = self.maybe_module()?.as_ref().ok()?;
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

pub fn to_hover_text(result: &Resolved) -> String {
  match result {
    Resolved::Ok { specifier, .. } => match specifier.scheme() {
      "data" => "_(a data url)_".to_string(),
      "blob" => "_(a blob url)_".to_string(),
      _ => format!(
        "{}&#8203;{}",
        &specifier[..url::Position::AfterScheme],
        &specifier[url::Position::AfterScheme..],
      )
      .replace('@', "&#8203;@"),
    },
    Resolved::Err(_) => "_[errored]_".to_string(),
    Resolved::None => "_[missing]_".to_string(),
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

#[derive(Debug, Default)]
struct SpecifierResolver {
  cache: HttpCache,
  redirects: Mutex<HashMap<ModuleSpecifier, ModuleSpecifier>>,
}

impl SpecifierResolver {
  pub fn new(cache_path: &Path) -> Self {
    Self {
      cache: HttpCache::new(cache_path),
      redirects: Mutex::new(HashMap::new()),
    }
  }

  pub fn resolve(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let scheme = specifier.scheme();
    if !SUPPORTED_SCHEMES.contains(&scheme) {
      return None;
    }

    if scheme == "data" || scheme == "blob" || scheme == "file" {
      Some(specifier.clone())
    } else {
      let mut redirects = self.redirects.lock();
      if let Some(specifier) = redirects.get(specifier) {
        Some(specifier.clone())
      } else {
        let redirect = self.resolve_remote(specifier, 10)?;
        redirects.insert(specifier.clone(), redirect.clone());
        Some(redirect)
      }
    }
  }

  fn resolve_remote(
    &self,
    specifier: &ModuleSpecifier,
    redirect_limit: usize,
  ) -> Option<ModuleSpecifier> {
    let cache_filename = self.cache.get_cache_filename(specifier)?;
    if redirect_limit > 0 && cache_filename.is_file() {
      let headers = http_cache::Metadata::read(&cache_filename)
        .ok()
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
  /// Adds or updates a document by reading the document from the file system
  /// returning the document.
  fn refresh_document(
    &mut self,
    cache: &HttpCache,
    maybe_resolver: Option<&dyn deno_graph::source::Resolver>,
    specifier: &ModuleSpecifier,
  ) -> Option<Document> {
    let path = get_document_path(cache, specifier)?;
    let fs_version = calculate_fs_version(&path)?;
    let bytes = fs::read(path).ok()?;
    let doc = if specifier.scheme() == "file" {
      let maybe_charset =
        Some(text_encoding::detect_charset(&bytes).to_string());
      let content = get_source_from_bytes(bytes, maybe_charset).ok()?;
      Document::new(
        specifier.clone(),
        fs_version,
        None,
        content.into(),
        maybe_resolver,
      )
    } else {
      let cache_filename = cache.get_cache_filename(specifier)?;
      let specifier_metadata =
        http_cache::Metadata::read(&cache_filename).ok()?;
      let maybe_content_type =
        specifier_metadata.headers.get("content-type").cloned();
      let maybe_headers = Some(&specifier_metadata.headers);
      let (_, maybe_charset) = map_content_type(specifier, maybe_content_type);
      let content = get_source_from_bytes(bytes, maybe_charset).ok()?;
      Document::new(
        specifier.clone(),
        fs_version,
        maybe_headers,
        content.into(),
        maybe_resolver,
      )
    };
    self.dirty = true;
    self.docs.insert(specifier.clone(), doc.clone());
    Some(doc)
  }
}

fn get_document_path(
  cache: &HttpCache,
  specifier: &ModuleSpecifier,
) -> Option<PathBuf> {
  if specifier.scheme() == "file" {
    specifier_to_file_path(specifier).ok()
  } else {
    cache.get_cache_filename(specifier)
  }
}

#[derive(Debug, Clone, Default)]
pub struct Documents {
  /// The DENO_DIR that the documents looks for non-file based modules.
  cache: HttpCache,
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
  /// Any imports to the context supplied by configuration files. This is like
  /// the imports into the a module graph in CLI.
  imports: Arc<HashMap<ModuleSpecifier, GraphImport>>,
  /// The optional import map that should be used when resolving dependencies.
  maybe_import_map: Option<ImportMapResolver>,
  /// The optional JSX resolver, which is used when JSX imports are configured.
  maybe_jsx_resolver: Option<JsxResolver>,
  /// Resolves a specifier to its final redirected to specifier.
  specifier_resolver: Arc<SpecifierResolver>,
}

impl Documents {
  pub fn new(location: &Path) -> Self {
    Self {
      cache: HttpCache::new(location),
      dirty: true,
      dependents_map: Default::default(),
      open_docs: HashMap::default(),
      file_system_docs: Default::default(),
      imports: Default::default(),
      maybe_import_map: None,
      maybe_jsx_resolver: None,
      specifier_resolver: Arc::new(SpecifierResolver::new(location)),
    }
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
    let maybe_resolver = self.get_maybe_resolver();
    let document = Document::open(
      specifier.clone(),
      version,
      language_id,
      content,
      maybe_resolver,
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
      .map_or_else(
        || {
          Err(custom_error(
            "NotFound",
            format!("The specifier \"{}\" was not found.", specifier),
          ))
        },
        Ok,
      )?;
    self.dirty = true;
    let doc = doc.with_change(version, changes, self.get_maybe_resolver())?;
    self.open_docs.insert(doc.specifier().clone(), doc.clone());
    Ok(doc)
  }

  /// Close an open document, this essentially clears any editor state that is
  /// being held, and the document store will revert to the file system if
  /// information about the document is required.
  pub fn close(&mut self, specifier: &ModuleSpecifier) -> Result<(), AnyError> {
    if self.open_docs.remove(specifier).is_some() {
      self.dirty = true;
    } else {
      let mut file_system_docs = self.file_system_docs.lock();
      if file_system_docs.docs.remove(specifier).is_some() {
        file_system_docs.dirty = true;
      } else {
        return Err(custom_error(
          "NotFound",
          format!("The specifier \"{}\" was not found.", specifier),
        ));
      }
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
    let maybe_resolver = self.get_maybe_resolver();
    let maybe_specifier = if let Some(resolver) = maybe_resolver {
      resolver.resolve(specifier, referrer).to_result().ok()
    } else {
      deno_core::resolve_import(specifier, referrer.as_str()).ok()
    };
    if let Some(import_specifier) = maybe_specifier {
      self.exists(&import_specifier)
    } else {
      false
    }
  }

  /// Return `true` if the specifier can be resolved to a document.
  pub fn exists(&self, specifier: &ModuleSpecifier) -> bool {
    // keep this fast because it's used by op_exists, which is a hot path in tsc
    let specifier = self.specifier_resolver.resolve(specifier);
    if let Some(specifier) = specifier {
      if self.open_docs.contains_key(&specifier) {
        return true;
      }
      if let Some(path) = get_document_path(&self.cache, &specifier) {
        return path.is_file();
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
    if let Some(specifier) = self.specifier_resolver.resolve(specifier) {
      recurse_dependents(&specifier, &self.dependents_map, &mut dependents);
      dependents.into_iter().collect()
    } else {
      vec![]
    }
  }

  /// Return a document for the specifier.
  pub fn get(&self, original_specifier: &ModuleSpecifier) -> Option<Document> {
    let specifier = self.specifier_resolver.resolve(original_specifier)?;
    if let Some(document) = self.open_docs.get(&specifier) {
      Some(document.clone())
    } else {
      let mut file_system_docs = self.file_system_docs.lock();
      let fs_version = get_document_path(&self.cache, &specifier)
        .and_then(|path| calculate_fs_version(&path));
      let file_system_doc = file_system_docs.docs.get(&specifier);
      if file_system_doc.map(|d| d.fs_version().to_string()) != fs_version {
        // attempt to update the file on the file system
        file_system_docs.refresh_document(
          &self.cache,
          self.get_maybe_resolver(),
          &specifier,
        )
      } else {
        file_system_doc.cloned()
      }
    }
  }

  /// Return a vector of documents that are contained in the document store,
  /// where `open_only` flag would provide only those documents currently open
  /// in the editor and `diagnosable_only` would provide only those documents
  /// that the language server can provide diagnostics for.
  pub fn documents(
    &self,
    open_only: bool,
    diagnosable_only: bool,
  ) -> Vec<Document> {
    if open_only {
      self
        .open_docs
        .values()
        .filter_map(|doc| {
          if !diagnosable_only || doc.is_diagnosable() {
            Some(doc.clone())
          } else {
            None
          }
        })
        .collect()
    } else {
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

  /// For a given set of string specifiers, resolve each one from the graph,
  /// for a given referrer. This is used to provide resolution information to
  /// tsc when type checking.
  pub fn resolve(
    &self,
    specifiers: Vec<String>,
    referrer: &ModuleSpecifier,
  ) -> Option<Vec<Option<(ModuleSpecifier, MediaType)>>> {
    let dependencies = self.get(referrer)?.0.dependencies.clone();
    let mut results = Vec::new();
    for specifier in specifiers {
      if specifier.starts_with("asset:") {
        if let Ok(specifier) = ModuleSpecifier::parse(&specifier) {
          let media_type = MediaType::from(&specifier);
          results.push(Some((specifier, media_type)));
        } else {
          results.push(None);
        }
      } else if let Some(dep) = dependencies.get(&specifier) {
        if let Resolved::Ok { specifier, .. } = &dep.maybe_type {
          results.push(self.resolve_dependency(specifier));
        } else if let Resolved::Ok { specifier, .. } = &dep.maybe_code {
          results.push(self.resolve_dependency(specifier));
        } else {
          results.push(None);
        }
      } else if let Some(Resolved::Ok { specifier, .. }) =
        self.resolve_imports_dependency(&specifier)
      {
        // clone here to avoid double borrow of self
        let specifier = specifier.clone();
        results.push(self.resolve_dependency(&specifier));
      } else {
        results.push(None);
      }
    }
    Some(results)
  }

  /// Update the location of the on disk cache for the document store.
  pub fn set_location(&mut self, location: &Path) {
    // TODO update resolved dependencies?
    self.cache = HttpCache::new(location);
    self.specifier_resolver = Arc::new(SpecifierResolver::new(location));
    self.dirty = true;
  }

  /// Tries to cache a navigation tree that is associated with the provided specifier
  /// if the document stored has the same script version.
  pub fn try_cache_navigation_tree(
    &mut self,
    specifier: &ModuleSpecifier,
    script_version: &str,
    navigation_tree: Arc<tsc::NavigationTree>,
  ) -> Result<(), AnyError> {
    if let Some(doc) = self.open_docs.get_mut(specifier) {
      if doc.script_version() == script_version {
        *doc = doc.with_navigation_tree(navigation_tree);
      }
    } else {
      let mut file_system_docs = self.file_system_docs.lock();
      if let Some(doc) = file_system_docs.docs.get_mut(specifier) {
        // ensure we are updating the same document
        // that the navigation tree was created for
        if doc.script_version() == script_version {
          *doc = doc.with_navigation_tree(navigation_tree);
        }
      } else {
        return Err(custom_error(
          "NotFound",
          format!("Specifier not found {}", specifier),
        ));
      }
    }
    Ok(())
  }

  pub fn update_config(
    &mut self,
    maybe_import_map: Option<Arc<import_map::ImportMap>>,
    maybe_config_file: Option<&ConfigFile>,
  ) {
    // TODO(@kitsonk) update resolved dependencies?
    self.maybe_import_map = maybe_import_map.map(ImportMapResolver::new);
    self.maybe_jsx_resolver = maybe_config_file.and_then(|cf| {
      cf.to_maybe_jsx_import_source_config()
        .map(|cfg| JsxResolver::new(cfg, self.maybe_import_map.clone()))
    });
    self.imports = Arc::new(
      if let Some(Ok(Some(imports))) =
        maybe_config_file.map(|cf| cf.to_maybe_imports())
      {
        imports
          .into_iter()
          .map(|(referrer, dependencies)| {
            let graph_import = GraphImport::new(
              referrer.clone(),
              dependencies,
              self.get_maybe_resolver(),
            );
            (referrer, graph_import)
          })
          .collect()
      } else {
        HashMap::new()
      },
    );
    self.dirty = true;
  }

  /// Iterate through the documents, building a map where the key is a unique
  /// document and the value is a set of specifiers that depend on that
  /// document.
  fn calculate_dependents_if_dirty(&mut self) {
    let mut file_system_docs = self.file_system_docs.lock();
    if !file_system_docs.dirty && !self.dirty {
      return;
    }

    let mut dependents_map: HashMap<ModuleSpecifier, HashSet<ModuleSpecifier>> =
      HashMap::new();
    // favour documents that are open in case a document exists in both collections
    let documents = file_system_docs.docs.iter().chain(self.open_docs.iter());
    for (specifier, doc) in documents {
      if let Some(Ok(module)) = doc.maybe_module() {
        for dependency in module.dependencies.values() {
          if let Some(dep) = dependency.get_code() {
            dependents_map
              .entry(dep.clone())
              .or_default()
              .insert(specifier.clone());
          }
          if let Some(dep) = dependency.get_type() {
            dependents_map
              .entry(dep.clone())
              .or_default()
              .insert(specifier.clone());
          }
        }
        if let Some((_, Resolved::Ok { specifier: dep, .. })) =
          &module.maybe_types_dependency
        {
          dependents_map
            .entry(dep.clone())
            .or_default()
            .insert(specifier.clone());
        }
      }
    }
    self.dependents_map = Arc::new(dependents_map);
    self.dirty = false;
    file_system_docs.dirty = false;
  }

  fn get_maybe_resolver(&self) -> Option<&dyn deno_graph::source::Resolver> {
    if self.maybe_jsx_resolver.is_some() {
      self.maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
    } else {
      self.maybe_import_map.as_ref().map(|im| im.as_resolver())
    }
  }

  fn resolve_dependency(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    let doc = self.get(specifier)?;
    let maybe_module = doc.maybe_module().and_then(|r| r.as_ref().ok());
    let maybe_types_dependency = maybe_module.and_then(|m| {
      m.maybe_types_dependency
        .as_ref()
        .map(|(_, resolved)| resolved.clone())
    });
    if let Some(Resolved::Ok { specifier, .. }) = maybe_types_dependency {
      self.resolve_dependency(&specifier)
    } else {
      let media_type = doc.media_type();
      Some((specifier.clone(), media_type))
    }
  }

  /// Iterate through any "imported" modules, checking to see if a dependency
  /// is available. This is used to provide "global" imports like the JSX import
  /// source.
  fn resolve_imports_dependency(
    &self,
    specifier: &str,
  ) -> Option<&deno_graph::Resolved> {
    for graph_imports in self.imports.values() {
      let maybe_dep = graph_imports.dependencies.get(specifier);
      if maybe_dep.is_some() {
        return maybe_dep.map(|d| &d.maybe_type);
      }
    }
    None
  }
}

/// The default parser from `deno_graph` does not include the configuration
/// options we require for the lsp.
#[derive(Debug, Default)]
struct LspModuleParser;

impl deno_graph::ModuleParser for LspModuleParser {
  fn parse_module(
    &self,
    specifier: &deno_graph::ModuleSpecifier,
    source: Arc<str>,
    media_type: MediaType,
  ) -> deno_core::anyhow::Result<ParsedSource, deno_ast::Diagnostic> {
    deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.to_string(),
      text_info: SourceTextInfo::new(source),
      media_type,
      capture_tokens: true,
      scope_analysis: true,
      maybe_syntax: None,
    })
  }
}

fn lsp_deno_graph_analyze(
  specifier: &ModuleSpecifier,
  content: Arc<str>,
  maybe_headers: Option<&HashMap<String, String>>,
  maybe_resolver: Option<&dyn deno_graph::source::Resolver>,
) -> (MaybeModuleResult, MaybeParsedSourceResult) {
  use deno_graph::ModuleParser;

  let analyzer = deno_graph::CapturingModuleAnalyzer::new(
    Some(Box::new(LspModuleParser::default())),
    None,
  );
  let parsed_source_result = analyzer.parse_module(
    specifier,
    content.clone(),
    MediaType::from_specifier_and_headers(specifier, maybe_headers),
  );
  let module_result = match &parsed_source_result {
    Ok(_) => deno_graph::parse_module(
      specifier,
      maybe_headers,
      content,
      Some(&deno_graph::ModuleKind::Esm),
      maybe_resolver,
      Some(&analyzer),
    ),
    Err(err) => Err(deno_graph::ModuleGraphError::ParseErr(
      specifier.clone(),
      err.clone(),
    )),
  };

  (Some(module_result), Some(parsed_source_result))
}

#[cfg(test)]
mod tests {
  use super::*;
  use test_util::TempDir;

  fn setup(temp_dir: &TempDir) -> (Documents, PathBuf) {
    let location = temp_dir.path().join("deps");
    let documents = Documents::new(&location);
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
    fs::create_dir_all(&documents_path).unwrap();
    fs::write(&file_path, "").unwrap();

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
    assert_eq!(documents.documents(false, false).len(), 1);
  }
}
