// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::future::Future;
use std::ops::Range;
use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Weak;
use std::time::SystemTime;

use dashmap::DashMap;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::future::Shared;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::RwLock;
use deno_core::resolve_url;
use deno_core::url::Position;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_error::JsErrorBox;
use deno_graph::TypesDependency;
use deno_path_util::url_to_file_path;
use deno_runtime::deno_node;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;
use indexmap::IndexMap;
use indexmap::IndexSet;
use lsp_types::Uri;
use node_resolver::cache::NodeResolutionThreadLocalCache;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use once_cell::sync::Lazy;
use serde::Serialize;
use tower_lsp::lsp_types as lsp;
use weak_table::PtrWeakKeyHashMap;
use weak_table::WeakValueHashMap;

use super::cache::calculate_fs_version_at_path;
use super::cache::LspCache;
use super::config::Config;
use super::logging::lsp_warn;
use super::resolver::LspResolver;
use super::resolver::ScopeDepInfo;
use super::resolver::SingleReferrerGraphResolver;
use super::testing::TestCollector;
use super::testing::TestModule;
use super::text::LineIndex;
use super::tsc::ChangeKind;
use super::tsc::NavigationTree;
use super::urls::uri_is_file_like;
use super::urls::uri_to_file_path;
use super::urls::uri_to_url;
use super::urls::url_to_uri;
use super::urls::COMPONENT;
use crate::graph_util::CliJsrUrlProvider;

#[derive(Debug)]
pub struct OpenDocument {
  pub uri: Arc<Uri>,
  pub text: Arc<str>,
  pub line_index: Arc<LineIndex>,
  pub version: i32,
  pub language_id: LanguageId,
  pub notebook_uri: Option<Arc<Uri>>,
  pub fs_version_on_open: Option<String>,
}

impl OpenDocument {
  fn new(
    uri: Uri,
    version: i32,
    language_id: LanguageId,
    text: Arc<str>,
    notebook_uri: Option<Arc<Uri>>,
  ) -> Self {
    let line_index = Arc::new(LineIndex::new(&text));
    let fs_version_on_open = uri_to_file_path(&uri)
      .ok()
      .and_then(calculate_fs_version_at_path);
    OpenDocument {
      uri: Arc::new(uri),
      text,
      line_index,
      version,
      language_id,
      notebook_uri,
      fs_version_on_open,
    }
  }

  fn with_change(
    &self,
    version: i32,
    changes: Vec<lsp::TextDocumentContentChangeEvent>,
  ) -> Result<Self, AnyError> {
    let mut text = self.text.to_string();
    let mut line_index = self.line_index.clone();
    let mut index_valid = IndexValid::All;
    for change in changes {
      if let Some(range) = change.range {
        if !index_valid.covers(range.start.line) {
          line_index = Arc::new(LineIndex::new(&text));
        }
        index_valid = IndexValid::UpTo(range.start.line);
        let range = line_index.get_text_range(range)?;
        text.replace_range(Range::<usize>::from(range), &change.text);
      } else {
        text = change.text;
        index_valid = IndexValid::UpTo(0);
      }
    }
    let text: Arc<str> = text.into();
    let line_index = if index_valid == IndexValid::All {
      line_index
    } else {
      Arc::new(LineIndex::new(&text))
    };
    Ok(OpenDocument {
      uri: self.uri.clone(),
      text,
      line_index,
      version,
      language_id: self.language_id,
      notebook_uri: self.notebook_uri.clone(),
      fs_version_on_open: self.fs_version_on_open.clone(),
    })
  }

  pub fn is_diagnosable(&self) -> bool {
    self.language_id.is_diagnosable()
  }

  pub fn is_file_like(&self) -> bool {
    uri_is_file_like(&self.uri)
  }

  pub fn script_version(&self) -> String {
    let fs_version = self.fs_version_on_open.as_deref().unwrap_or("1");
    format!("{fs_version}+{}", self.version)
  }
}

fn remote_url_to_uri(url: &Url) -> Option<Uri> {
  if !matches!(url.scheme(), "http" | "https") {
    return None;
  }
  let mut string = String::with_capacity(url.as_str().len() + 6);
  string.push_str("deno:/");
  string.push_str(url.scheme());
  for p in url[Position::BeforeHost..].split('/') {
    string.push('/');
    string.push_str(
      &percent_encoding::utf8_percent_encode(p, COMPONENT).to_string(),
    );
  }
  Uri::from_str(&string)
    .inspect_err(|err| {
      lsp_warn!("Couldn't convert remote URL \"{url}\" to URI: {err}")
    })
    .ok()
}

fn asset_url_to_uri(url: &Url) -> Option<Uri> {
  if url.scheme() != "asset" {
    return None;
  }
  Uri::from_str(&format!("deno:/asset{}", url.path()))
    .inspect_err(|err| {
      lsp_warn!("Couldn't convert asset URL \"{url}\" to URI: {err}")
    })
    .ok()
}

fn data_url_to_uri(url: &Url) -> Option<Uri> {
  let data_url = deno_media_type::data_url::RawDataUrl::parse(url).ok()?;
  let media_type = data_url.media_type();
  let extension = if media_type == MediaType::Unknown {
    ""
  } else {
    media_type.as_ts_extension()
  };
  let mut file_name_str = url.path().to_string();
  if let Some(query) = url.query() {
    file_name_str.push('?');
    file_name_str.push_str(query);
  }
  let hash = deno_lib::util::checksum::gen(&[file_name_str.as_bytes()]);
  Uri::from_str(&format!("deno:/data_url/{hash}{extension}",))
    .inspect_err(|err| {
      lsp_warn!("Couldn't convert data url \"{url}\" to URI: {err}")
    })
    .ok()
}

#[derive(Debug, Clone)]
pub enum DocumentText {
  Static(&'static str),
  Arc(Arc<str>),
}

impl DocumentText {
  /// Will clone the string if static.
  pub fn to_arc(&self) -> Arc<str> {
    match self {
      Self::Static(s) => (*s).into(),
      Self::Arc(s) => s.clone(),
    }
  }
}

impl std::ops::Deref for DocumentText {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    match self {
      Self::Static(s) => s,
      Self::Arc(s) => s,
    }
  }
}

impl Serialize for DocumentText {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    (self as &str).serialize(serializer)
  }
}

#[derive(Debug, Clone)]
pub enum ServerDocumentKind {
  Fs {
    fs_version: String,
    text: Arc<str>,
  },
  RemoteUrl {
    url: Arc<Url>,
    fs_cache_version: String,
    text: Arc<str>,
  },
  DataUrl {
    url: Arc<Url>,
    text: Arc<str>,
  },
  Asset {
    url: Arc<Url>,
    text: &'static str,
  },
}

#[derive(Debug)]
pub struct ServerDocument {
  pub uri: Arc<Uri>,
  pub media_type: MediaType,
  pub line_index: Arc<LineIndex>,
  pub kind: ServerDocumentKind,
}

impl ServerDocument {
  fn load(uri: &Uri) -> Option<Self> {
    let scheme = uri.scheme()?;
    if scheme.eq_lowercase("file") {
      let url = uri_to_url(uri);
      let path = url_to_file_path(&url).ok()?;
      let bytes = fs::read(&path).ok()?;
      let media_type = MediaType::from_specifier(&url);
      let text: Arc<str> =
        bytes_to_content(&url, media_type, bytes, None).ok()?.into();
      let fs_version = calculate_fs_version_at_path(&path)?;
      let line_index = Arc::new(LineIndex::new(&text));
      return Some(Self {
        uri: Arc::new(uri.clone()),
        media_type,
        line_index,
        kind: ServerDocumentKind::Fs { fs_version, text },
      });
    }
    None
  }

  fn remote_url(
    uri: &Uri,
    url: Arc<Url>,
    scope: Option<&Url>,
    cache: &LspCache,
  ) -> Option<Self> {
    let media_type = MediaType::from_specifier(&url);
    let http_cache = cache.for_specifier(scope);
    let cache_key = http_cache.cache_item_key(&url).ok()?;
    let cache_entry = http_cache.get(&cache_key, None).ok()??;
    let (_, maybe_charset) =
      deno_graph::source::resolve_media_type_and_charset_from_headers(
        &url,
        Some(&cache_entry.metadata.headers),
      );
    let fs_cache_version = (|| {
      let modified = http_cache.read_modified_time(&cache_key).ok()??;
      let duration = modified.duration_since(SystemTime::UNIX_EPOCH).ok()?;
      Some(duration.as_millis().to_string())
    })()
    .unwrap_or_else(|| "1".to_string());
    let text: Arc<str> = bytes_to_content(
      &url,
      media_type,
      cache_entry.content.into_owned(),
      maybe_charset,
    )
    .ok()?
    .into();
    let line_index = Arc::new(LineIndex::new(&text));
    Some(Self {
      uri: Arc::new(uri.clone()),
      media_type,
      line_index,
      kind: ServerDocumentKind::RemoteUrl {
        url,
        fs_cache_version,
        text,
      },
    })
  }

  fn asset(name: &str, text: &'static str) -> Self {
    let url = Arc::new(Url::parse(&format!("asset:///{name}")).unwrap());
    let uri = asset_url_to_uri(&url).unwrap();
    let media_type = MediaType::from_specifier(&url);
    let line_index = Arc::new(LineIndex::new(text));
    Self {
      uri: Arc::new(uri),
      media_type,
      line_index,
      kind: ServerDocumentKind::Asset { url, text },
    }
  }

  fn data_url(uri: &Uri, url: Arc<Url>) -> Option<Self> {
    let raw_data_url =
      deno_media_type::data_url::RawDataUrl::parse(&url).ok()?;
    let media_type = raw_data_url.media_type();
    let text: Arc<str> = raw_data_url.decode().ok()?.into();
    let line_index = Arc::new(LineIndex::new(&text));
    Some(Self {
      uri: Arc::new(uri.clone()),
      media_type,
      line_index,
      kind: ServerDocumentKind::DataUrl { url, text },
    })
  }

  pub fn text(&self) -> DocumentText {
    match &self.kind {
      ServerDocumentKind::Fs { text, .. } => DocumentText::Arc(text.clone()),
      ServerDocumentKind::RemoteUrl { text, .. } => {
        DocumentText::Arc(text.clone())
      }
      ServerDocumentKind::DataUrl { text, .. } => {
        DocumentText::Arc(text.clone())
      }
      ServerDocumentKind::Asset { text, .. } => DocumentText::Static(text),
    }
  }

  pub fn is_diagnosable(&self) -> bool {
    media_type_is_diagnosable(self.media_type)
  }

  pub fn is_file_like(&self) -> bool {
    uri_is_file_like(&self.uri)
  }

  pub fn script_version(&self) -> String {
    match &self.kind {
      ServerDocumentKind::Fs { fs_version, .. } => fs_version.clone(),
      ServerDocumentKind::RemoteUrl {
        fs_cache_version, ..
      } => fs_cache_version.clone(),
      ServerDocumentKind::DataUrl { .. } => "1".to_string(),
      ServerDocumentKind::Asset { .. } => "1".to_string(),
    }
  }
}

#[derive(Debug)]
pub struct AssetDocuments {
  inner: HashMap<Arc<Uri>, Arc<ServerDocument>>,
}

impl AssetDocuments {
  pub fn get(&self, k: &Uri) -> Option<&Arc<ServerDocument>> {
    self.inner.get(k)
  }
}

pub static ASSET_DOCUMENTS: Lazy<AssetDocuments> =
  Lazy::new(|| AssetDocuments {
    inner: crate::tsc::LAZILY_LOADED_STATIC_ASSETS
      .iter()
      .map(|(k, v)| {
        let doc = Arc::new(ServerDocument::asset(k, v.as_str()));
        let uri = doc.uri.clone();
        (uri, doc)
      })
      .collect(),
  });

#[derive(Debug, Clone)]
pub enum Document {
  Open(Arc<OpenDocument>),
  Server(Arc<ServerDocument>),
}

impl Document {
  pub fn open(&self) -> Option<&Arc<OpenDocument>> {
    match self {
      Self::Open(d) => Some(d),
      Self::Server(_) => None,
    }
  }

  pub fn server(&self) -> Option<&Arc<ServerDocument>> {
    match self {
      Self::Open(_) => None,
      Self::Server(d) => Some(d),
    }
  }

  pub fn uri(&self) -> &Arc<Uri> {
    match self {
      Self::Open(d) => &d.uri,
      Self::Server(d) => &d.uri,
    }
  }

  pub fn text(&self) -> DocumentText {
    match self {
      Self::Open(d) => DocumentText::Arc(d.text.clone()),
      Self::Server(d) => d.text(),
    }
  }

  pub fn line_index(&self) -> &Arc<LineIndex> {
    match self {
      Self::Open(d) => &d.line_index,
      Self::Server(d) => &d.line_index,
    }
  }

  pub fn script_version(&self) -> String {
    match self {
      Self::Open(d) => d.script_version(),
      Self::Server(d) => d.script_version(),
    }
  }

  pub fn is_diagnosable(&self) -> bool {
    match self {
      Self::Open(d) => d.is_diagnosable(),
      Self::Server(d) => d.is_diagnosable(),
    }
  }

  pub fn is_file_like(&self) -> bool {
    match self {
      Self::Open(d) => d.is_file_like(),
      Self::Server(d) => d.is_file_like(),
    }
  }
}

#[derive(Debug, Default, Clone)]
pub struct Documents {
  open: IndexMap<Uri, Arc<OpenDocument>>,
  server: Arc<DashMap<Uri, Arc<ServerDocument>>>,
  cells_by_notebook_uri: BTreeMap<Arc<Uri>, Vec<Arc<Uri>>>,
  file_like_uris_by_url: Arc<DashMap<Url, Arc<Uri>>>,
  /// These URLs can not be recovered from the URIs we assign them without these
  /// maps. We want to be able to discard old documents from here but keep these
  /// mappings.
  data_urls_by_uri: Arc<DashMap<Uri, Arc<Url>>>,
  remote_urls_by_uri: Arc<DashMap<Uri, Arc<Url>>>,
}

impl Documents {
  fn open(
    &mut self,
    uri: Uri,
    version: i32,
    language_id: LanguageId,
    text: Arc<str>,
    notebook_uri: Option<Arc<Uri>>,
  ) -> Arc<OpenDocument> {
    self.server.remove(&uri);
    let doc = Arc::new(OpenDocument::new(
      uri.clone(),
      version,
      language_id,
      text,
      notebook_uri,
    ));
    self.open.insert(uri, doc.clone());
    if !doc.uri.scheme().is_some_and(|s| s.eq_lowercase("file")) {
      let url = uri_to_url(&doc.uri);
      if url.scheme() == "file" {
        self.file_like_uris_by_url.insert(url, doc.uri.clone());
      }
    }
    doc
  }

  fn change(
    &mut self,
    uri: &Uri,
    version: i32,
    changes: Vec<lsp::TextDocumentContentChangeEvent>,
  ) -> Result<Arc<OpenDocument>, AnyError> {
    let Some((uri, doc)) = self.open.shift_remove_entry(uri) else {
      return Err(
        JsErrorBox::new(
          "NotFound",
          format!(
            "The URI \"{}\" does not refer to an open document.",
            uri.as_str()
          ),
        )
        .into(),
      );
    };
    let doc = Arc::new(doc.with_change(version, changes)?);
    self.open.insert(uri, doc.clone());
    Ok(doc)
  }

  fn close(&mut self, uri: &Uri) -> Result<Arc<OpenDocument>, AnyError> {
    self.file_like_uris_by_url.retain(|_, u| u.as_ref() != uri);
    let doc = self.open.shift_remove(uri).ok_or_else(|| {
      JsErrorBox::new(
        "NotFound",
        format!(
          "The URI \"{}\" does not refer to an open document.",
          uri.as_str()
        ),
      )
    })?;
    Ok(doc)
  }

  fn open_notebook(
    &mut self,
    uri: Uri,
    cells: Vec<lsp::TextDocumentItem>,
  ) -> Vec<Arc<OpenDocument>> {
    let uri = Arc::new(uri);
    let mut documents = Vec::with_capacity(cells.len());
    for cell in cells {
      let language_id = cell.language_id.parse().unwrap_or_else(|err| {
        lsp_warn!("{:#}", err);
        LanguageId::Unknown
      });
      if language_id == LanguageId::Unknown {
        lsp_warn!(
          "Unsupported language id \"{}\" received for document \"{}\".",
          cell.language_id,
          cell.uri.as_str()
        );
      }
      let document = self.open(
        cell.uri.clone(),
        cell.version,
        language_id,
        cell.text.into(),
        Some(uri.clone()),
      );
      documents.push(document);
    }
    self
      .cells_by_notebook_uri
      .insert(uri, documents.iter().map(|d| d.uri.clone()).collect());
    documents
  }

  pub fn change_notebook(
    &mut self,
    uri: &Uri,
    structure: Option<lsp::NotebookDocumentCellChangeStructure>,
    content: Option<Vec<lsp::NotebookDocumentChangeTextContent>>,
  ) -> Vec<(Arc<OpenDocument>, ChangeKind)> {
    let uri = Arc::new(uri.clone());
    let mut documents_with_change_kinds = Vec::new();
    if let Some(structure) = structure {
      if let Some(cells) = self.cells_by_notebook_uri.get_mut(&uri) {
        cells.splice(
          structure.array.start as usize
            ..(structure.array.start + structure.array.delete_count) as usize,
          structure
            .array
            .cells
            .into_iter()
            .flatten()
            .map(|c| Arc::new(c.document)),
        );
      }
      for closed in structure.did_close.into_iter().flatten() {
        let document = match self.close(&closed.uri) {
          Ok(d) => d,
          Err(err) => {
            lsp_warn!("{:#}", err);
            continue;
          }
        };
        documents_with_change_kinds.push((document, ChangeKind::Closed));
      }
      for opened in structure.did_open.into_iter().flatten() {
        let language_id = opened.language_id.parse().unwrap_or_else(|err| {
          lsp_warn!("{:#}", err);
          LanguageId::Unknown
        });
        if language_id == LanguageId::Unknown {
          lsp_warn!(
            "Unsupported language id \"{}\" received for document \"{}\".",
            opened.language_id,
            opened.uri.as_str()
          );
        }
        let document = self.open(
          opened.uri,
          opened.version,
          language_id,
          opened.text.into(),
          Some(uri.clone()),
        );
        documents_with_change_kinds.push((document, ChangeKind::Opened));
      }
    }
    for changed in content.into_iter().flatten() {
      let document = match self.change(
        &changed.document.uri,
        changed.document.version,
        changed.changes,
      ) {
        Ok(d) => d,
        Err(err) => {
          lsp_warn!("{:#}", err);
          continue;
        }
      };
      documents_with_change_kinds.push((document, ChangeKind::Modified));
    }
    documents_with_change_kinds
  }

  pub fn close_notebook(&mut self, uri: &Uri) -> Vec<Arc<OpenDocument>> {
    let Some(cell_uris) = self.cells_by_notebook_uri.remove(uri) else {
      lsp_warn!(
        "The URI \"{}\" does not refer to an open notebook document.",
        uri.as_str(),
      );
      return Default::default();
    };
    let mut documents = Vec::with_capacity(cell_uris.len());
    for cell_uri in cell_uris {
      let document = match self.close(&cell_uri) {
        Ok(d) => d,
        Err(err) => {
          lsp_warn!("{:#}", err);
          continue;
        }
      };
      documents.push(document);
    }
    documents
  }

  pub fn get(&self, uri: &Uri) -> Option<Document> {
    if let Some(doc) = self.open.get(uri) {
      return Some(Document::Open(doc.clone()));
    }
    if let Some(doc) = ASSET_DOCUMENTS.get(uri) {
      return Some(Document::Server(doc.clone()));
    }
    if let Some(doc) = self.server.get(uri) {
      return Some(Document::Server(doc.clone()));
    }
    let doc = if let Some(doc) = ServerDocument::load(uri) {
      doc
    } else if let Some(data_url) = self.data_urls_by_uri.get(uri) {
      ServerDocument::data_url(uri, data_url.value().clone())?
    } else {
      return None;
    };
    let doc = Arc::new(doc);
    self.server.insert(uri.clone(), doc.clone());
    Some(Document::Server(doc))
  }

  /// This will not create any server entries, only retrieve existing entries.
  pub fn inspect(&self, uri: &Uri) -> Option<Document> {
    if let Some(doc) = self.open.get(uri) {
      return Some(Document::Open(doc.clone()));
    }
    if let Some(doc) = self.server.get(uri) {
      return Some(Document::Server(doc.clone()));
    }
    None
  }

  pub fn get_for_specifier(
    &self,
    specifier: &Url,
    scope: Option<&Url>,
    cache: &LspCache,
  ) -> Option<Document> {
    let scheme = specifier.scheme();
    if scheme == "file" {
      let uri = self
        .file_like_uris_by_url
        .get(specifier)
        .map(|e| e.value().clone())
        .or_else(|| url_to_uri(specifier).ok().map(Arc::new))?;
      self.get(&uri)
    } else if scheme == "asset" {
      let uri = asset_url_to_uri(specifier)?;
      self.get(&uri)
    } else if scheme == "http" || scheme == "https" {
      if let Some(vendored_specifier) =
        cache.vendored_specifier(specifier, scope)
      {
        let uri = url_to_uri(&vendored_specifier).ok()?;
        self.get(&uri)
      } else {
        let uri = remote_url_to_uri(specifier)?;
        if let Some(doc) = self.server.get(&uri) {
          return Some(Document::Server(doc.clone()));
        }
        let url = Arc::new(specifier.clone());
        self.remote_urls_by_uri.insert(uri.clone(), url.clone());
        let doc =
          Arc::new(ServerDocument::remote_url(&uri, url, scope, cache)?);
        self.server.insert(uri, doc.clone());
        Some(Document::Server(doc))
      }
    } else if scheme == "data" {
      let uri = data_url_to_uri(specifier)?;
      if let Some(doc) = self.server.get(&uri) {
        return Some(Document::Server(doc.clone()));
      }
      let url = Arc::new(specifier.clone());
      self.data_urls_by_uri.insert(uri.clone(), url.clone());
      let doc = Arc::new(ServerDocument::data_url(&uri, url)?);
      self.server.insert(uri, doc.clone());
      Some(Document::Server(doc))
    } else {
      None
    }
  }

  pub fn cells_by_notebook_uri(&self) -> &BTreeMap<Arc<Uri>, Vec<Arc<Uri>>> {
    &self.cells_by_notebook_uri
  }

  pub fn open_docs(&self) -> impl Iterator<Item = &Arc<OpenDocument>> {
    self.open.values()
  }

  pub fn server_docs(&self) -> Vec<Arc<ServerDocument>> {
    self.server.iter().map(|e| e.value().clone()).collect()
  }

  pub fn docs(&self) -> Vec<Document> {
    self
      .open
      .values()
      .map(|d| Document::Open(d.clone()))
      .chain(
        self
          .server
          .iter()
          .map(|e| Document::Server(e.value().clone())),
      )
      .collect()
  }

  pub fn filtered_docs(
    &self,
    predicate: impl FnMut(&Document) -> bool,
  ) -> Vec<Document> {
    self
      .open
      .values()
      .map(|d| Document::Open(d.clone()))
      .chain(
        self
          .server
          .iter()
          .map(|e| Document::Server(e.value().clone())),
      )
      .filter(predicate)
      .collect()
  }

  pub fn remove_server_doc(&self, uri: &Uri) {
    self.server.remove(uri);
  }
}

#[derive(Debug)]
pub struct DocumentModuleOpenData {
  pub version: i32,
  pub parsed_source: Option<ParsedSourceResult>,
}

#[derive(Debug)]
pub struct DocumentModule {
  pub uri: Arc<Uri>,
  pub open_data: Option<DocumentModuleOpenData>,
  pub notebook_uri: Option<Arc<Uri>>,
  pub script_version: String,
  pub specifier: Arc<Url>,
  pub scope: Option<Arc<Url>>,
  pub media_type: MediaType,
  pub headers: Option<HashMap<String, String>>,
  pub text: DocumentText,
  pub line_index: Arc<LineIndex>,
  pub resolution_mode: ResolutionMode,
  pub dependencies: Arc<IndexMap<String, deno_graph::Dependency>>,
  pub types_dependency: Option<Arc<TypesDependency>>,
  pub navigation_tree: tokio::sync::OnceCell<Arc<NavigationTree>>,
  pub semantic_tokens_full: tokio::sync::OnceCell<lsp::SemanticTokens>,
  text_info_cell: once_cell::sync::OnceCell<SourceTextInfo>,
  test_module_fut: Option<TestModuleFut>,
}

impl DocumentModule {
  pub fn new(
    document: &Document,
    specifier: Arc<Url>,
    scope: Option<Arc<Url>>,
    resolver: &LspResolver,
    config: &Config,
    cache: &LspCache,
  ) -> Self {
    let text = document.text();
    let headers = matches!(specifier.scheme(), "http" | "https")
      .then(|| {
        let http_cache = cache.for_specifier(scope.as_deref());
        let cache_key = http_cache.cache_item_key(&specifier).ok()?;
        let cache_entry = http_cache.get(&cache_key, None).ok()??;
        Some(cache_entry.metadata.headers)
      })
      .flatten();
    let open_document = document.open();
    let media_type = resolve_media_type(
      &specifier,
      headers.as_ref(),
      open_document.map(|d| d.language_id),
    );
    let (parsed_source, maybe_module, resolution_mode) =
      if media_type_is_diagnosable(media_type) {
        parse_and_analyze_module(
          specifier.as_ref().clone(),
          text.to_arc(),
          headers.as_ref(),
          media_type,
          scope.as_deref(),
          resolver,
        )
      } else {
        (None, None, ResolutionMode::Import)
      };
    let maybe_module = maybe_module.and_then(Result::ok);
    let dependencies = maybe_module
      .as_ref()
      .map(|m| Arc::new(m.dependencies.clone()))
      .unwrap_or_default();
    let types_dependency = maybe_module
      .as_ref()
      .and_then(|m| Some(Arc::new(m.maybe_types_dependency.clone()?)));
    let test_module_fut =
      get_maybe_test_module_fut(parsed_source.as_ref(), config);
    DocumentModule {
      uri: document.uri().clone(),
      open_data: open_document.map(|d| DocumentModuleOpenData {
        version: d.version,
        parsed_source,
      }),
      notebook_uri: open_document.and_then(|d| d.notebook_uri.clone()),
      script_version: document.script_version(),
      specifier,
      scope,
      media_type,
      headers,
      text,
      line_index: document.line_index().clone(),
      resolution_mode,
      dependencies,
      types_dependency,
      navigation_tree: Default::default(),
      semantic_tokens_full: Default::default(),
      text_info_cell: Default::default(),
      test_module_fut,
    }
  }

  pub fn is_diagnosable(&self) -> bool {
    media_type_is_diagnosable(self.media_type)
  }

  pub fn dependency_at_position(
    &self,
    position: &lsp::Position,
  ) -> Option<(&str, &deno_graph::Dependency, &deno_graph::Range)> {
    let position = deno_graph::Position {
      line: position.line as usize,
      character: position.character as usize,
    };
    self
      .dependencies
      .iter()
      .find_map(|(s, dep)| dep.includes(position).map(|r| (s.as_str(), dep, r)))
  }

  pub fn text_info(&self) -> &SourceTextInfo {
    // try to get the text info from the parsed source and if
    // not then create one in the cell
    self
      .open_data
      .as_ref()
      .and_then(|d| d.parsed_source.as_ref())
      .and_then(|p| p.as_ref().ok())
      .map(|p| p.text_info_lazy())
      .unwrap_or_else(|| {
        self
          .text_info_cell
          .get_or_init(|| SourceTextInfo::new(self.text.to_arc()))
      })
  }

  pub async fn test_module(&self) -> Option<Arc<TestModule>> {
    self.test_module_fut.clone()?.await
  }
}

type DepInfoByScope = BTreeMap<Option<Arc<Url>>, Arc<ScopeDepInfo>>;

#[derive(Debug, Default)]
struct WeakDocumentModuleMap {
  open: RwLock<PtrWeakKeyHashMap<Weak<OpenDocument>, Arc<DocumentModule>>>,
  server: RwLock<PtrWeakKeyHashMap<Weak<ServerDocument>, Arc<DocumentModule>>>,
  by_specifier: RwLock<WeakValueHashMap<Arc<Url>, Weak<DocumentModule>>>,
}

impl WeakDocumentModuleMap {
  fn get(&self, document: &Document) -> Option<Arc<DocumentModule>> {
    match document {
      Document::Open(d) => self.open.read().get(d).cloned(),
      Document::Server(d) => self.server.read().get(d).cloned(),
    }
  }

  fn get_for_specifier(&self, specifier: &Url) -> Option<Arc<DocumentModule>> {
    self.by_specifier.read().get(specifier)
  }

  fn contains_specifier(&self, specifier: &Url) -> bool {
    self.by_specifier.read().contains_key(specifier)
  }

  fn inspect_values(&self) -> Vec<Arc<DocumentModule>> {
    self
      .open
      .read()
      .values()
      .cloned()
      .chain(self.server.read().values().cloned())
      .collect()
  }

  fn insert(
    &self,
    document: &Document,
    module: Arc<DocumentModule>,
  ) -> Option<Arc<DocumentModule>> {
    match document {
      Document::Open(d) => {
        self.open.write().insert(d.clone(), module.clone());
      }
      Document::Server(d) => {
        self.server.write().insert(d.clone(), module.clone());
      }
    }
    self
      .by_specifier
      .write()
      .insert(module.specifier.clone(), module.clone());
    Some(module)
  }

  fn remove_expired(&self) {
    // IMPORTANT: Maintain this order based on weak ref relations.
    self.open.write().remove_expired();
    self.server.write().remove_expired();
    self.by_specifier.write().remove_expired();
  }
}

#[derive(Debug, Default, Clone)]
pub struct DocumentModules {
  pub documents: Documents,
  config: Arc<Config>,
  resolver: Arc<LspResolver>,
  cache: Arc<LspCache>,
  workspace_files: Arc<IndexSet<PathBuf>>,
  dep_info_by_scope: once_cell::sync::OnceCell<Arc<DepInfoByScope>>,
  modules_unscoped: Arc<WeakDocumentModuleMap>,
  modules_by_scope: Arc<BTreeMap<Arc<Url>, Arc<WeakDocumentModuleMap>>>,
}

impl DocumentModules {
  pub fn update_config(
    &mut self,
    config: &Config,
    resolver: &Arc<LspResolver>,
    cache: &LspCache,
    workspace_files: &Arc<IndexSet<PathBuf>>,
  ) {
    self.config = Arc::new(config.clone());
    self.cache = Arc::new(cache.clone());
    self.resolver = resolver.clone();
    self.workspace_files = workspace_files.clone();
    self.modules_unscoped = Default::default();
    self.modules_by_scope = Arc::new(
      self
        .config
        .tree
        .data_by_scope()
        .keys()
        .map(|s| (s.clone(), Default::default()))
        .collect(),
    );
    self.dep_info_by_scope = Default::default();

    node_resolver::PackageJsonThreadLocalCache::clear();
    NodeResolutionThreadLocalCache::clear();

    // Clean up non-existent documents.
    self.documents.server.retain(|_, d| {
      let Some(module) =
        self.inspect_primary_module(&Document::Server(d.clone()))
      else {
        return false;
      };
      let Ok(path) = url_to_file_path(&module.specifier) else {
        // Remove non-file schemed docs (deps). They may not be dependencies
        // anymore after updating resolvers.
        return false;
      };
      if !config.specifier_enabled(&module.specifier) {
        return false;
      }
      path.is_file()
    });
  }

  pub fn open_document(
    &mut self,
    uri: Uri,
    version: i32,
    language_id: LanguageId,
    text: Arc<str>,
    notebook_uri: Option<Arc<Uri>>,
  ) -> Arc<OpenDocument> {
    self.dep_info_by_scope = Default::default();
    self
      .documents
      .open(uri, version, language_id, text, notebook_uri)
  }

  pub fn change_document(
    &mut self,
    uri: &Uri,
    version: i32,
    changes: Vec<lsp::TextDocumentContentChangeEvent>,
  ) -> Result<Arc<OpenDocument>, AnyError> {
    self.dep_info_by_scope = Default::default();
    let document = self.documents.change(uri, version, changes)?;
    Ok(document)
  }

  /// Returns if the document is diagnosable.
  pub fn close_document(
    &mut self,
    uri: &Uri,
  ) -> Result<Arc<OpenDocument>, AnyError> {
    self.dep_info_by_scope = Default::default();
    let document = self.documents.close(uri)?;
    // If applicable, try to load the closed document as a server document so
    // it's still included as a ts root etc..
    if uri.scheme().is_some_and(|s| s.eq_lowercase("file"))
      && self.config.uri_enabled(uri)
    {
      self.documents.get(uri);
    }
    Ok(document)
  }

  pub fn open_notebook_document(
    &mut self,
    uri: Uri,
    cells: Vec<lsp::TextDocumentItem>,
  ) -> Vec<Arc<OpenDocument>> {
    self.dep_info_by_scope = Default::default();
    self.documents.open_notebook(uri, cells)
  }

  pub fn change_notebook_document(
    &mut self,
    uri: &Uri,
    structure: Option<lsp::NotebookDocumentCellChangeStructure>,
    content: Option<Vec<lsp::NotebookDocumentChangeTextContent>>,
  ) -> Vec<(Arc<OpenDocument>, ChangeKind)> {
    self.dep_info_by_scope = Default::default();
    self.documents.change_notebook(uri, structure, content)
  }

  pub fn close_notebook_document(
    &mut self,
    uri: &Uri,
  ) -> Vec<Arc<OpenDocument>> {
    self.dep_info_by_scope = Default::default();
    self.documents.close_notebook(uri)
  }

  pub fn release(&self, specifier: &Url, scope: Option<&Url>) {
    let Some(module) = self.module_for_specifier(specifier, scope) else {
      return;
    };
    self.documents.remove_server_doc(&module.uri);
  }

  fn infer_specifier(&self, document: &Document) -> Option<Arc<Url>> {
    if let Some(document) = document.server() {
      match &document.kind {
        ServerDocumentKind::Fs { .. } => {}
        ServerDocumentKind::RemoteUrl { url, .. } => return Some(url.clone()),
        ServerDocumentKind::DataUrl { url, .. } => return Some(url.clone()),
        ServerDocumentKind::Asset { url, .. } => return Some(url.clone()),
      }
    }
    let uri = document.uri();
    let url = uri_to_url(uri);
    if url.scheme() != "file" {
      return None;
    }
    if uri.scheme().is_some_and(|s| s.eq_lowercase("file")) {
      if let Some(remote_specifier) = self.cache.unvendored_specifier(&url) {
        return Some(Arc::new(remote_specifier));
      }
    }
    Some(Arc::new(url))
  }

  fn module_inner(
    &self,
    document: &Document,
    specifier: Option<&Arc<Url>>,
    scope: Option<&Url>,
  ) -> Option<Arc<DocumentModule>> {
    let modules = self.modules_for_scope(scope)?;
    if let Some(module) = modules.get(document) {
      return Some(module);
    }
    let specifier = specifier
      .cloned()
      .or_else(|| self.infer_specifier(document))?;
    let module = Arc::new(DocumentModule::new(
      document,
      specifier,
      scope.cloned().map(Arc::new),
      &self.resolver,
      &self.config,
      &self.cache,
    ));
    modules.insert(document, module.clone());
    Some(module)
  }

  pub fn module(
    &self,
    document: &Document,
    scope: Option<&Url>,
  ) -> Option<Arc<DocumentModule>> {
    self.module_inner(document, None, scope)
  }

  pub fn module_for_specifier(
    &self,
    specifier: &Url,
    scope: Option<&Url>,
  ) -> Option<Arc<DocumentModule>> {
    let scoped_resolver = self.resolver.get_scoped_resolver(scope);
    let specifier = if let Ok(jsr_req_ref) =
      JsrPackageReqReference::from_specifier(specifier)
    {
      Cow::Owned(scoped_resolver.jsr_to_resource_url(&jsr_req_ref)?)
    } else {
      Cow::Borrowed(specifier)
    };
    let specifier = scoped_resolver.resolve_redirects(&specifier)?;
    let document =
      self
        .documents
        .get_for_specifier(&specifier, scope, &self.cache)?;
    self.module_inner(&document, Some(&Arc::new(specifier)), scope)
  }

  pub fn primary_module(
    &self,
    document: &Document,
  ) -> Option<Arc<DocumentModule>> {
    if let Some(scope) = self.primary_scope(document.uri()) {
      return self.module(document, scope.map(|s| s.as_ref()));
    }
    for modules in self.modules_by_scope.values() {
      if let Some(module) = modules.get(document) {
        return Some(module);
      }
    }
    self.modules_unscoped.get(document)
  }

  pub fn workspace_file_modules_by_scope(
    &self,
  ) -> BTreeMap<Option<Arc<Url>>, Vec<Arc<DocumentModule>>> {
    let mut modules_with_scopes = BTreeMap::new();
    for path in self
      .workspace_files
      .iter()
      .take(self.config.settings.unscoped.document_preload_limit)
    {
      let Ok(url) = Url::from_file_path(path) else {
        continue;
      };
      let scope = self.config.tree.scope_for_specifier(&url).cloned();
      let Some(document) =
        self
          .documents
          .get_for_specifier(&url, scope.as_deref(), &self.cache)
      else {
        continue;
      };
      if document.open().is_none()
        && (!self.config.specifier_enabled(&url)
          || self.resolver.in_node_modules(&url)
          || self.cache.in_cache_directory(&url))
      {
        continue;
      }
      let Some(module) = self.module(&document, scope.as_deref()) else {
        continue;
      };
      modules_with_scopes.insert(document.uri().clone(), (module, scope));
    }
    // Include files that aren't in `self.workspace_files` for whatever reason.
    for document in self.documents.docs() {
      let uri = document.uri();
      if modules_with_scopes.contains_key(uri) {
        continue;
      }
      let open_document = document.open();
      if open_document.is_some_and(|d| d.notebook_uri.is_some()) {
        continue;
      }
      let url = uri_to_url(uri);
      if open_document.is_none()
        && (url.scheme() != "file"
          || !self.config.specifier_enabled(&url)
          || self.resolver.in_node_modules(&url)
          || self.cache.in_cache_directory(&url))
      {
        continue;
      }
      let scope = self.config.tree.scope_for_specifier(&url).cloned();
      let Some(module) = self.module(&document, scope.as_deref()) else {
        continue;
      };
      modules_with_scopes.insert(document.uri().clone(), (module, scope));
    }
    let mut result = BTreeMap::new();
    for (module, scope) in modules_with_scopes.into_values() {
      (result.entry(scope).or_default() as &mut Vec<_>).push(module);
    }
    result
  }

  /// This will not create any module entries, only retrieve existing entries.
  pub fn inspect_module_for_specifier(
    &self,
    specifier: &Url,
    scope: Option<&Url>,
  ) -> Option<Arc<DocumentModule>> {
    let scoped_resolver = self.resolver.get_scoped_resolver(scope);
    let specifier = if let Ok(jsr_req_ref) =
      JsrPackageReqReference::from_specifier(specifier)
    {
      Cow::Owned(scoped_resolver.jsr_to_resource_url(&jsr_req_ref)?)
    } else {
      Cow::Borrowed(specifier)
    };
    let specifier = scoped_resolver.resolve_redirects(&specifier)?;
    let modules = self.modules_for_scope(scope)?;
    modules.get_for_specifier(&specifier)
  }

  /// This will not create any module entries, only retrieve existing entries.
  pub fn inspect_primary_module(
    &self,
    document: &Document,
  ) -> Option<Arc<DocumentModule>> {
    if let Some(scope) = self.primary_scope(document.uri()) {
      return self
        .modules_for_scope(scope.map(|s| s.as_ref()))?
        .get(document);
    }
    for modules in self.modules_by_scope.values() {
      if let Some(module) = modules.get(document) {
        return Some(module);
      }
    }
    self.modules_unscoped.get(document)
  }

  /// This will not store any module entries, only retrieve existing entries or
  /// create temporary entries for scopes where one doesn't exist.
  pub fn inspect_or_temp_modules_by_scope(
    &self,
    document: &Document,
  ) -> BTreeMap<Option<Arc<Url>>, Arc<DocumentModule>> {
    let mut result = BTreeMap::new();
    for (scope, modules) in self.modules_by_scope.iter() {
      let module = modules.get(document).unwrap_or_else(|| {
        Arc::new(DocumentModule::new(
          document,
          Arc::new(uri_to_url(document.uri())),
          Some(scope.clone()),
          &self.resolver,
          &self.config,
          &self.cache,
        ))
      });
      result.insert(Some(scope.clone()), module);
    }
    let module = self.modules_unscoped.get(document).unwrap_or_else(|| {
      Arc::new(DocumentModule::new(
        document,
        Arc::new(uri_to_url(document.uri())),
        None,
        &self.resolver,
        &self.config,
        &self.cache,
      ))
    });
    result.insert(None, module);
    result
  }

  fn modules_for_scope(
    &self,
    scope: Option<&Url>,
  ) -> Option<&Arc<WeakDocumentModuleMap>> {
    match scope {
      Some(s) => Some(self.modules_by_scope.get(s)?),
      None => Some(&self.modules_unscoped),
    }
  }

  pub fn primary_scope(&self, uri: &Uri) -> Option<Option<&Arc<Url>>> {
    let url = uri_to_url(uri);
    if url.scheme() == "file" && !self.cache.in_global_cache_directory(&url) {
      let scope = self.config.tree.scope_for_specifier(&url);
      return Some(scope);
    }
    None
  }

  pub fn primary_specifier(&self, document: &Document) -> Option<Arc<Url>> {
    self
      .inspect_primary_module(document)
      .map(|m| m.specifier.clone())
      .or_else(|| self.infer_specifier(document))
  }

  pub fn remove_expired_modules(&self) {
    self.modules_unscoped.remove_expired();
    for modules in self.modules_by_scope.values() {
      modules.remove_expired();
    }
  }

  pub fn scopes(&self) -> BTreeSet<Option<Arc<Url>>> {
    self
      .modules_by_scope
      .keys()
      .cloned()
      .map(Some)
      .chain([None])
      .collect()
  }

  pub fn specifier_exists(&self, specifier: &Url, scope: Option<&Url>) -> bool {
    if let Some(modules) = self.modules_for_scope(scope) {
      if modules.contains_specifier(specifier) {
        return true;
      }
    }
    if specifier.scheme() == "file" {
      return url_to_file_path(specifier)
        .map(|p| p.is_file())
        .unwrap_or(false);
    }
    if specifier.scheme() == "data" {
      return true;
    }
    if self.cache.for_specifier(scope).contains(specifier) {
      return true;
    }
    false
  }

  pub fn dep_info_by_scope(
    &self,
  ) -> Arc<BTreeMap<Option<Arc<Url>>, Arc<ScopeDepInfo>>> {
    type ScopeEntry<'a> =
      (Option<&'a Arc<Url>>, &'a Arc<WeakDocumentModuleMap>);
    let dep_info_from_scope_entry = |(scope, modules): ScopeEntry<'_>| {
      let mut dep_info = ScopeDepInfo::default();
      let mut visit_module = |module: &DocumentModule| {
        for dependency in module.dependencies.values() {
          let code_specifier = dependency.get_code();
          let type_specifier = dependency.get_type();
          if let Some(dep) = code_specifier {
            if dep.scheme() == "node" {
              dep_info.has_node_specifier = true;
            }
            if let Ok(reference) = NpmPackageReqReference::from_specifier(dep) {
              dep_info.npm_reqs.insert(reference.into_inner().req);
            }
          }
          if let Some(dep) = type_specifier {
            if let Ok(reference) = NpmPackageReqReference::from_specifier(dep) {
              dep_info.npm_reqs.insert(reference.into_inner().req);
            }
          }
          if dependency.maybe_deno_types_specifier.is_some() {
            if let (Some(code_specifier), Some(type_specifier)) =
              (code_specifier, type_specifier)
            {
              if MediaType::from_specifier(type_specifier).is_declaration() {
                dep_info
                  .deno_types_to_code_resolutions
                  .insert(type_specifier.clone(), code_specifier.clone());
              }
            }
          }
        }
        if let Some(dep) = module
          .types_dependency
          .as_ref()
          .and_then(|d| d.dependency.maybe_specifier())
        {
          if let Ok(reference) = NpmPackageReqReference::from_specifier(dep) {
            dep_info.npm_reqs.insert(reference.into_inner().req);
          }
        }
      };
      for module in modules.inspect_values() {
        visit_module(&module);
      }
      let config_data =
        scope.and_then(|s| self.config.tree.data_by_scope().get(s));
      if let Some(config_data) = config_data {
        (|| {
          let member_dir = &config_data.member_dir;
          let jsx_config =
            member_dir.to_maybe_jsx_import_source_config().ok()??;
          let import_source_types = jsx_config.import_source_types.as_ref()?;
          let import_source = jsx_config.import_source.as_ref()?;
          let scoped_resolver =
            self.resolver.get_scoped_resolver(scope.map(|s| s.as_ref()));
          let cli_resolver = scoped_resolver.as_cli_resolver();
          let type_specifier = cli_resolver
            .resolve(
              &import_source_types.specifier,
              &import_source_types.base,
              deno_graph::Position::zeroed(),
              // todo(dsherret): this is wrong because it doesn't consider CJS referrers
              ResolutionMode::Import,
              NodeResolutionKind::Types,
            )
            .ok()?;
          let code_specifier = cli_resolver
            .resolve(
              &import_source.specifier,
              &import_source.base,
              deno_graph::Position::zeroed(),
              // todo(dsherret): this is wrong because it doesn't consider CJS referrers
              ResolutionMode::Import,
              NodeResolutionKind::Execution,
            )
            .ok()?;
          dep_info
            .deno_types_to_code_resolutions
            .insert(type_specifier, code_specifier);
          Some(())
        })();
        // fill the reqs from the lockfile
        if let Some(lockfile) = config_data.lockfile.as_ref() {
          let lockfile = lockfile.lock();
          for dep_req in lockfile.content.packages.specifiers.keys() {
            if dep_req.kind == deno_semver::package::PackageKind::Npm {
              dep_info.npm_reqs.insert(dep_req.req.clone());
            }
          }
        }
      }
      if dep_info.has_node_specifier
        && !dep_info.npm_reqs.iter().any(|r| r.name == "@types/node")
      {
        dep_info
          .npm_reqs
          .insert(PackageReq::from_str("@types/node").unwrap());
      }
      (scope.cloned(), Arc::new(dep_info))
    };
    self
      .dep_info_by_scope
      .get_or_init(|| {
        NodeResolutionThreadLocalCache::clear();
        // Ensure at least module entries for workspace files are initialized.
        self.workspace_file_modules_by_scope();
        Arc::new(
          self
            .modules_by_scope
            .iter()
            .map(|(s, m)| (Some(s), m))
            .chain([(None, &self.modules_unscoped)])
            .map(dep_info_from_scope_entry)
            .collect(),
        )
      })
      .clone()
  }

  pub fn scopes_with_node_specifier(&self) -> HashSet<Option<Arc<Url>>> {
    self
      .dep_info_by_scope()
      .iter()
      .filter(|(_, i)| i.has_node_specifier)
      .map(|(s, _)| s.clone())
      .collect::<HashSet<_>>()
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  pub fn resolve(
    &self,
    // (is_cjs: bool, raw_specifier: String)
    raw_specifiers: &[(bool, String)],
    referrer: &Url,
    scope: Option<&Url>,
  ) -> Vec<Option<(Url, MediaType)>> {
    let referrer_module = self.module_for_specifier(referrer, scope);
    let dependencies = referrer_module.as_ref().map(|d| &d.dependencies);
    let mut results = Vec::new();
    let scoped_resolver = self.resolver.get_scoped_resolver(scope);
    for (is_cjs, raw_specifier) in raw_specifiers {
      let resolution_mode = match is_cjs {
        true => ResolutionMode::Require,
        false => ResolutionMode::Import,
      };
      if raw_specifier.starts_with("asset:") {
        if let Ok(specifier) = resolve_url(raw_specifier) {
          let media_type = MediaType::from_specifier(&specifier);
          results.push(Some((specifier, media_type)));
        } else {
          results.push(None);
        }
      } else if let Some(dep) =
        dependencies.as_ref().and_then(|d| d.get(raw_specifier))
      {
        if let Some(specifier) = dep.maybe_type.maybe_specifier() {
          results.push(self.resolve_dependency(
            specifier,
            referrer,
            resolution_mode,
            scope,
          ));
        } else if let Some(specifier) = dep.maybe_code.maybe_specifier() {
          results.push(self.resolve_dependency(
            specifier,
            referrer,
            resolution_mode,
            scope,
          ));
        } else {
          results.push(None);
        }
      } else if let Ok(specifier) = scoped_resolver.as_cli_resolver().resolve(
        raw_specifier,
        referrer,
        deno_graph::Position::zeroed(),
        resolution_mode,
        NodeResolutionKind::Types,
      ) {
        results.push(self.resolve_dependency(
          &specifier,
          referrer,
          resolution_mode,
          scope,
        ));
      } else {
        results.push(None);
      }
    }
    results
  }

  #[cfg_attr(feature = "lsp-tracing", tracing::instrument(skip_all))]
  pub fn resolve_dependency(
    &self,
    specifier: &Url,
    referrer: &Url,
    resolution_mode: ResolutionMode,
    scope: Option<&Url>,
  ) -> Option<(Url, MediaType)> {
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
      let scoped_resolver = self.resolver.get_scoped_resolver(scope);
      let (s, mt) =
        scoped_resolver.npm_to_file_url(&npm_ref, referrer, resolution_mode)?;
      specifier = s;
      media_type = Some(mt);
    }
    let Some(module) = self.module_for_specifier(&specifier, scope) else {
      let media_type =
        media_type.unwrap_or_else(|| MediaType::from_specifier(&specifier));
      return Some((specifier, media_type));
    };
    if let Some(types) = module
      .types_dependency
      .as_ref()
      .and_then(|d| d.dependency.maybe_specifier())
    {
      self.resolve_dependency(types, &specifier, module.resolution_mode, scope)
    } else {
      Some((module.specifier.as_ref().clone(), module.media_type))
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
  JavaScript,
  Jsx,
  TypeScript,
  Tsx,
  Json,
  JsonC,
  Markdown,
  Html,
  Css,
  Scss,
  Sass,
  Less,
  Yaml,
  Sql,
  Svelte,
  Vue,
  Astro,
  Vento,
  Nunjucks,
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
      LanguageId::Html => Some("html"),
      LanguageId::Css => Some("css"),
      LanguageId::Scss => Some("scss"),
      LanguageId::Sass => Some("sass"),
      LanguageId::Less => Some("less"),
      LanguageId::Yaml => Some("yaml"),
      LanguageId::Sql => Some("sql"),
      LanguageId::Svelte => Some("svelte"),
      LanguageId::Vue => Some("vue"),
      LanguageId::Astro => Some("astro"),
      LanguageId::Vento => Some("vto"),
      LanguageId::Nunjucks => Some("njk"),
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
      LanguageId::Html => Some("text/html"),
      LanguageId::Css => Some("text/css"),
      LanguageId::Scss => None,
      LanguageId::Sass => None,
      LanguageId::Less => None,
      LanguageId::Yaml => Some("application/yaml"),
      LanguageId::Sql => None,
      LanguageId::Svelte => None,
      LanguageId::Vue => None,
      LanguageId::Astro => None,
      LanguageId::Vento => None,
      LanguageId::Nunjucks => None,
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
      "html" => Ok(Self::Html),
      "css" => Ok(Self::Css),
      "scss" => Ok(Self::Scss),
      "sass" => Ok(Self::Sass),
      "less" => Ok(Self::Less),
      "yaml" => Ok(Self::Yaml),
      "sql" => Ok(Self::Sql),
      "svelte" => Ok(Self::Svelte),
      "vue" => Ok(Self::Vue),
      "astro" => Ok(Self::Astro),
      "vento" => Ok(Self::Vento),
      "nunjucks" => Ok(Self::Nunjucks),
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
  if specifier.scheme() != "file"
    || specifier.as_str().contains("/node_modules/")
  {
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
    parsed_source.program().visit_with(&mut collector);
    Arc::new(collector.take())
  })
  .map(Result::ok)
  .boxed()
  .shared();
  Some(handle)
}

fn resolve_media_type(
  specifier: &ModuleSpecifier,
  maybe_headers: Option<&HashMap<String, String>>,
  maybe_language_id: Option<LanguageId>,
) -> MediaType {
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

/// Loader that will look at the open documents.
pub struct OpenDocumentsGraphLoader<'a> {
  pub inner_loader: &'a mut dyn deno_graph::source::Loader,
  pub open_modules: &'a HashMap<Arc<Url>, Arc<DocumentModule>>,
}

impl OpenDocumentsGraphLoader<'_> {
  fn load_from_docs(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<deno_graph::source::LoadFuture> {
    if specifier.scheme() == "file" {
      if let Some(doc) = self.open_modules.get(specifier) {
        return Some(
          future::ready(Ok(Some(deno_graph::source::LoadResponse::Module {
            content: Arc::from(doc.text.as_bytes().to_owned()),
            specifier: doc.specifier.as_ref().clone(),
            maybe_headers: None,
          })))
          .boxed_local(),
        );
      }
    }
    None
  }
}

impl deno_graph::source::Loader for OpenDocumentsGraphLoader<'_> {
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
    media_type: MediaType,
    source: &Arc<[u8]>,
    module_info: &deno_graph::ModuleInfo,
  ) {
    self.inner_loader.cache_module_info(
      specifier,
      media_type,
      source,
      module_info,
    )
  }
}

fn parse_and_analyze_module(
  specifier: ModuleSpecifier,
  text: Arc<str>,
  maybe_headers: Option<&HashMap<String, String>>,
  media_type: MediaType,
  file_referrer: Option<&ModuleSpecifier>,
  resolver: &LspResolver,
) -> (
  Option<ParsedSourceResult>,
  Option<ModuleResult>,
  ResolutionMode,
) {
  let parsed_source_result = parse_source(specifier.clone(), text, media_type);
  let (module_result, resolution_mode) = analyze_module(
    specifier,
    &parsed_source_result,
    maybe_headers,
    file_referrer,
    resolver,
  );
  (
    Some(parsed_source_result),
    Some(module_result),
    resolution_mode,
  )
}

fn parse_source(
  specifier: ModuleSpecifier,
  text: Arc<str>,
  media_type: MediaType,
) -> ParsedSourceResult {
  deno_ast::parse_program(deno_ast::ParseParams {
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
) -> (ModuleResult, ResolutionMode) {
  match parsed_source_result {
    Ok(parsed_source) => {
      let scoped_resolver = resolver.get_scoped_resolver(file_referrer);
      let npm_resolver = scoped_resolver.as_graph_npm_resolver();
      let cli_resolver = scoped_resolver.as_cli_resolver();
      let is_cjs_resolver = scoped_resolver.as_is_cjs_resolver();
      let config_data = scoped_resolver.as_config_data();
      let valid_referrer = specifier.clone();
      let jsx_import_source_config =
        config_data.and_then(|d| d.maybe_jsx_import_source_config());
      let module_resolution_mode = is_cjs_resolver.get_lsp_resolution_mode(
        &specifier,
        Some(parsed_source.compute_is_script()),
      );
      let resolver = SingleReferrerGraphResolver {
        valid_referrer: &valid_referrer,
        module_resolution_mode,
        cli_resolver,
        jsx_import_source_config: jsx_import_source_config.as_ref(),
      };
      (
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
            maybe_resolver: Some(&resolver),
            maybe_npm_resolver: Some(npm_resolver.as_ref()),
          },
        )),
        module_resolution_mode,
      )
    }
    Err(err) => (
      Err(deno_graph::ModuleGraphError::ModuleError(
        deno_graph::ModuleError::ParseErr(specifier, err.clone()),
      )),
      ResolutionMode::Import,
    ),
  }
}

fn bytes_to_content(
  specifier: &ModuleSpecifier,
  media_type: MediaType,
  bytes: Vec<u8>,
  maybe_charset: Option<&str>,
) -> Result<String, AnyError> {
  if media_type == MediaType::Wasm {
    // we use the dts representation for Wasm modules
    Ok(deno_graph::source::wasm::wasm_module_to_dts(&bytes)?)
  } else {
    let charset = maybe_charset.unwrap_or_else(|| {
      deno_media_type::encoding::detect_charset(specifier, &bytes)
    });
    Ok(deno_media_type::encoding::decode_owned_source(
      charset, bytes,
    )?)
  }
}

#[cfg(test)]
mod tests {
  use deno_config::deno_json::ConfigFile;
  use deno_core::serde_json;
  use deno_core::serde_json::json;
  use pretty_assertions::assert_eq;
  use test_util::TempDir;

  use super::*;
  use crate::lsp::cache::LspCache;

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

  async fn setup() -> (DocumentModules, LspCache, TempDir) {
    let temp_dir = TempDir::new();
    temp_dir.create_dir_all(".deno_dir");
    let cache = LspCache::new(Some(temp_dir.url().join(".deno_dir").unwrap()));
    let config = Config::default();
    let resolver =
      Arc::new(LspResolver::from_config(&config, &cache, None).await);
    let mut document_modules = DocumentModules::default();
    document_modules.update_config(
      &config,
      &resolver,
      &cache,
      &Default::default(),
    );
    (document_modules, cache, temp_dir)
  }

  #[tokio::test]
  async fn test_documents_open_close() {
    let (mut document_modules, _, _) = setup().await;
    let uri = Uri::from_str("file:///a.ts").unwrap();
    let content = r#"import * as b from "./b.ts";
console.log(b);
"#;
    document_modules.open_document(
      uri.clone(),
      1,
      "javascript".parse().unwrap(),
      content.into(),
      None,
    );
    let document = document_modules
      .documents
      .get(&uri)
      .unwrap()
      .open()
      .cloned()
      .unwrap();
    assert_eq!(document.uri.as_ref(), &uri);
    assert_eq!(document.text.as_ref(), content);
    assert_eq!(document.version, 1);
    assert_eq!(document.language_id, LanguageId::JavaScript);
    assert!(document.is_diagnosable());
    assert!(document.is_file_like());
    document_modules.close_document(&uri).unwrap();
    assert!(document_modules.documents.get(&uri).is_none());
  }

  #[tokio::test]
  async fn test_documents_change() {
    let (mut document_modules, _, _) = setup().await;
    let uri = Uri::from_str("file:///a.ts").unwrap();
    let content = r#"import * as b from "./b.ts";
console.log(b);
"#;
    document_modules.open_document(
      uri.clone(),
      1,
      "javascript".parse().unwrap(),
      content.into(),
      None,
    );
    document_modules
      .change_document(
        &uri,
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
      document_modules
        .documents
        .get(&uri)
        .unwrap()
        .text()
        .as_ref() as &str,
      r#"import * as b from "./b.ts";
console.log(b, "hello deno");
"#
    );
  }

  #[tokio::test]
  async fn test_documents_refresh_dependencies_config_change() {
    // it should never happen that a user of this API causes this to happen,
    // but we'll guard against it anyway
    let (mut document_modules, cache, temp_dir) = setup().await;

    let file1_path = temp_dir.path().join("file1.ts");
    let file1_specifier = temp_dir.url().join("file1.ts").unwrap();
    fs::write(&file1_path, "").unwrap();

    let file2_path = temp_dir.path().join("file2.ts");
    let file2_specifier = temp_dir.url().join("file2.ts").unwrap();
    fs::write(&file2_path, "").unwrap();

    let file3_path = temp_dir.path().join("file3.ts");
    let file3_specifier = temp_dir.url().join("file3.ts").unwrap();
    fs::write(&file3_path, "").unwrap();

    let mut config = Config::new_with_roots([temp_dir.url()]);
    let workspace_settings =
      serde_json::from_str(r#"{ "enable": true }"#).unwrap();
    config.set_workspace_settings(workspace_settings, vec![]);
    let workspace_files = Arc::new(
      [&file1_specifier, &file2_specifier, &file3_specifier]
        .into_iter()
        .map(|s| s.to_file_path().unwrap())
        .collect::<IndexSet<_>>(),
    );

    let document = document_modules.open_document(
      url_to_uri(&file1_specifier).unwrap(),
      1,
      LanguageId::TypeScript,
      "import {} from 'test';".into(),
      None,
    );

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
            config.root_url().unwrap().join("deno.json").unwrap(),
          )
          .unwrap(),
          &default_registry(),
        )
        .await;

      let resolver =
        Arc::new(LspResolver::from_config(&config, &cache, None).await);
      document_modules.update_config(
        &config,
        &resolver,
        &cache,
        &workspace_files,
      );

      let module = document_modules
        .primary_module(&Document::Open(document.clone()))
        .unwrap();
      assert_eq!(
        module
          .dependencies
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
            config.root_url().unwrap().join("deno.json").unwrap(),
          )
          .unwrap(),
          &default_registry(),
        )
        .await;

      let resolver =
        Arc::new(LspResolver::from_config(&config, &cache, None).await);
      document_modules.update_config(
        &config,
        &resolver,
        &cache,
        &workspace_files,
      );

      // check the document's dependencies
      let module = document_modules
        .primary_module(&Document::Open(document.clone()))
        .unwrap();
      assert_eq!(
        module
          .dependencies
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
