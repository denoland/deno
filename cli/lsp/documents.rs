// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use super::text::LineIndex;
use super::tsc;

use crate::config_file::ConfigFile;
use crate::file_fetcher::get_source_from_bytes;
use crate::file_fetcher::map_content_type;
use crate::file_fetcher::SUPPORTED_SCHEMES;
use crate::http_cache;
use crate::http_cache::HttpCache;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;
use crate::text_encoding;

use deno_ast::MediaType;
use deno_ast::SourceTextInfo;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url;
use deno_core::ModuleSpecifier;
use lspower::lsp;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::ops::Range;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::SystemTime;

lazy_static::lazy_static! {
  static ref JS_HEADERS: HashMap<String, String> = ([
    ("content-type".to_string(), "application/javascript".to_string())
  ]).iter().cloned().collect();
  static ref JSX_HEADERS: HashMap<String, String> = ([
    ("content-type".to_string(), "text/jsx".to_string())
  ]).iter().cloned().collect();
  static ref TS_HEADERS: HashMap<String, String> = ([
    ("content-type".to_string(), "application/typescript".to_string())
  ]).iter().cloned().collect();
  static ref TSX_HEADERS: HashMap<String, String> = ([
    ("content-type".to_string(), "text/tsx".to_string())
  ]).iter().cloned().collect();
}

/// The default parser from `deno_graph` does not include the configuration
/// options we require here, and so implementing an empty struct that provides
/// the trait.
#[derive(Debug, Default)]
struct SourceParser {}

impl deno_graph::SourceParser for SourceParser {
  fn parse_module(
    &self,
    specifier: &ModuleSpecifier,
    source: Arc<String>,
    media_type: MediaType,
  ) -> Result<deno_ast::ParsedSource, deno_ast::Diagnostic> {
    deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.to_string(),
      source: SourceTextInfo::new(source),
      media_type,
      capture_tokens: true,
      scope_analysis: true,
      maybe_syntax: None,
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LanguageId {
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

// TODO(@kitsonk) expose the synthetic module from deno_graph
#[derive(Debug)]
struct SyntheticModule {
  dependencies: BTreeMap<String, deno_graph::Resolved>,
  specifier: ModuleSpecifier,
}

impl SyntheticModule {
  pub fn new(
    specifier: ModuleSpecifier,
    dependencies: Vec<(String, Option<lsp::Range>)>,
    maybe_resolver: Option<&dyn deno_graph::source::Resolver>,
  ) -> Self {
    let dependencies = dependencies
      .iter()
      .map(|(dep, maybe_range)| {
        let range = to_deno_graph_range(&specifier, maybe_range.as_ref());
        let result = if let Some(resolver) = maybe_resolver {
          resolver.resolve(dep, &specifier).map_err(|err| {
            if let Some(specifier_error) =
              err.downcast_ref::<deno_graph::SpecifierError>()
            {
              deno_graph::ResolutionError::InvalidSpecifier(
                specifier_error.clone(),
                range.clone(),
              )
            } else {
              deno_graph::ResolutionError::ResolverError(
                Arc::new(err),
                dep.to_string(),
                range.clone(),
              )
            }
          })
        } else {
          deno_core::resolve_import(dep, specifier.as_str()).map_err(|err| {
            deno_graph::ResolutionError::ResolverError(
              Arc::new(err.into()),
              dep.to_string(),
              range.clone(),
            )
          })
        };
        (dep.to_string(), Some(result.map(|s| (s, range))))
      })
      .collect();
    Self {
      dependencies,
      specifier,
    }
  }
}

#[derive(Debug)]
pub(crate) struct Document {
  line_index: Arc<LineIndex>,
  maybe_language_id: Option<LanguageId>,
  maybe_lsp_version: Option<i32>,
  maybe_module:
    Option<Result<deno_graph::Module, deno_graph::ModuleGraphError>>,
  maybe_navigation_tree: Option<Arc<tsc::NavigationTree>>,
  maybe_warning: Option<String>,
  source: SourceTextInfo,
  specifier: ModuleSpecifier,
  version: String,
}

impl Document {
  fn new(
    specifier: ModuleSpecifier,
    version: String,
    maybe_headers: Option<&HashMap<String, String>>,
    content: Arc<String>,
    maybe_resolver: Option<&dyn deno_graph::source::Resolver>,
  ) -> Self {
    let maybe_warning = maybe_headers
      .map(|h| h.get("x-deno-warning").cloned())
      .flatten();
    let parser = SourceParser::default();
    // we only ever do `Document::new` on on disk resources that are supposed to
    // be diagnosable, unlike `Document::open`, so it is safe to unconditionally
    // parse the module.
    let maybe_module = Some(deno_graph::parse_module(
      &specifier,
      maybe_headers,
      content.clone(),
      maybe_resolver,
      Some(&parser),
    ));
    let source = SourceTextInfo::new(content);
    let line_index = Arc::new(LineIndex::new(source.text_str()));
    Self {
      line_index,
      maybe_language_id: None,
      maybe_lsp_version: None,
      maybe_module,
      maybe_navigation_tree: None,
      maybe_warning,
      source,
      specifier,
      version,
    }
  }

  fn change(
    &mut self,
    version: i32,
    changes: Vec<lsp::TextDocumentContentChangeEvent>,
    maybe_resolver: Option<&dyn deno_graph::source::Resolver>,
  ) -> Result<(), AnyError> {
    let mut content = self.source.text_str().to_string();
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
    let content = Arc::new(content);
    if self
      .maybe_language_id
      .as_ref()
      .map(|li| li.is_diagnosable())
      .unwrap_or(false)
    {
      let maybe_headers = self
        .maybe_language_id
        .as_ref()
        .map(|li| li.as_headers())
        .flatten();
      let parser = SourceParser::default();
      self.maybe_module = Some(deno_graph::parse_module(
        &self.specifier,
        maybe_headers,
        content.clone(),
        maybe_resolver,
        Some(&parser),
      ));
    } else {
      self.maybe_module = None;
    }
    self.source = SourceTextInfo::new(content);
    self.line_index = if index_valid == IndexValid::All {
      line_index
    } else {
      Arc::new(LineIndex::new(self.source.text_str()))
    };
    self.maybe_lsp_version = Some(version);
    self.maybe_navigation_tree = None;
    Ok(())
  }

  fn close(&mut self) {
    self.maybe_lsp_version = None;
    self.maybe_language_id = None;
  }

  fn content(&self) -> Arc<String> {
    self.source.text()
  }

  fn is_diagnosable(&self) -> bool {
    matches!(
      self.media_type(),
      // todo(#12410): Update with new media types for TS 4.5
      MediaType::JavaScript
        | MediaType::Jsx
        | MediaType::TypeScript
        | MediaType::Tsx
        | MediaType::Dts
    )
  }

  fn is_open(&self) -> bool {
    self.maybe_lsp_version.is_some()
  }

  fn maybe_types_dependency(&self) -> deno_graph::Resolved {
    let module_result = self.maybe_module.as_ref()?;
    let module = module_result.as_ref().ok()?;
    let (_, maybe_dep) = module.maybe_types_dependency.as_ref()?;
    maybe_dep.clone()
  }

  fn media_type(&self) -> MediaType {
    if let Some(Ok(module)) = &self.maybe_module {
      module.media_type
    } else {
      MediaType::from(&self.specifier)
    }
  }

  fn open(
    specifier: ModuleSpecifier,
    version: i32,
    language_id: LanguageId,
    content: Arc<String>,
    maybe_resolver: Option<&dyn deno_graph::source::Resolver>,
  ) -> Self {
    let maybe_headers = language_id.as_headers();
    let parser = SourceParser::default();
    let maybe_module = if language_id.is_diagnosable() {
      Some(deno_graph::parse_module(
        &specifier,
        maybe_headers,
        content.clone(),
        maybe_resolver,
        Some(&parser),
      ))
    } else {
      None
    };
    let source = SourceTextInfo::new(content);
    let line_index = Arc::new(LineIndex::new(source.text_str()));
    Self {
      line_index,
      maybe_language_id: Some(language_id),
      maybe_lsp_version: Some(version),
      maybe_module,
      maybe_navigation_tree: None,
      maybe_warning: None,
      source,
      specifier,
      version: "1".to_string(),
    }
  }
}

pub(crate) fn to_hover_text(
  result: &Result<
    (ModuleSpecifier, deno_graph::Range),
    deno_graph::ResolutionError,
  >,
) -> String {
  match result {
    Ok((specifier, _)) => match specifier.scheme() {
      "data" => "_(a data url)_".to_string(),
      "blob" => "_(a blob url)_".to_string(),
      _ => format!(
        "{}&#8203;{}",
        specifier[..url::Position::AfterScheme].to_string(),
        specifier[url::Position::AfterScheme..].to_string()
      ),
    },
    Err(_) => "_[errored]_".to_string(),
  }
}

pub(crate) fn to_lsp_range(range: &deno_graph::Range) -> lsp::Range {
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

fn to_deno_graph_range(
  specifier: &ModuleSpecifier,
  maybe_range: Option<&lsp::Range>,
) -> deno_graph::Range {
  let specifier = specifier.clone();
  if let Some(range) = maybe_range {
    deno_graph::Range {
      specifier,
      start: deno_graph::Position {
        line: range.start.line as usize,
        character: range.start.character as usize,
      },
      end: deno_graph::Position {
        line: range.end.line as usize,
        character: range.end.character as usize,
      },
    }
  } else {
    deno_graph::Range {
      specifier,
      start: deno_graph::Position::zeroed(),
      end: deno_graph::Position::zeroed(),
    }
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
struct Inner {
  /// The DENO_DIR that the documents looks for non-file based modules.
  cache: HttpCache,
  /// A flag that indicates that stated data is potentially invalid and needs to
  /// be recalculated before being considered valid.
  dirty: bool,
  /// A map where the key is a specifier and the value is a set of specifiers
  /// that depend on the key.
  dependents_map: HashMap<ModuleSpecifier, HashSet<ModuleSpecifier>>,
  /// A map of documents that can either be "open" in the language server, or
  /// just present on disk.
  docs: HashMap<ModuleSpecifier, Document>,
  /// Any imports to the context supplied by configuration files. This is like
  /// the imports into the a module graph in CLI.
  imports: HashMap<ModuleSpecifier, SyntheticModule>,
  /// The optional import map that should be used when resolving dependencies.
  maybe_import_map: Option<ImportMapResolver>,
  /// The optional JSX resolver, which is used when JSX imports are configured.
  maybe_jsx_resolver: Option<JsxResolver>,
  redirects: HashMap<ModuleSpecifier, ModuleSpecifier>,
}

impl Inner {
  fn new(location: &Path) -> Self {
    Self {
      cache: HttpCache::new(location),
      dirty: true,
      dependents_map: HashMap::default(),
      docs: HashMap::default(),
      imports: HashMap::default(),
      maybe_import_map: None,
      maybe_jsx_resolver: None,
      redirects: HashMap::default(),
    }
  }

  /// Adds a document by reading the document from the file system.
  fn add(&mut self, specifier: ModuleSpecifier) -> Option<Document> {
    let version = self.calculate_version(&specifier)?;
    let path = self.get_path(&specifier)?;
    let bytes = fs::read(path).ok()?;
    let doc = if specifier.scheme() == "file" {
      let maybe_charset =
        Some(text_encoding::detect_charset(&bytes).to_string());
      let content = Arc::new(get_source_from_bytes(bytes, maybe_charset).ok()?);
      Document::new(
        specifier.clone(),
        version,
        None,
        content,
        self.get_maybe_resolver(),
      )
    } else {
      let cache_filename = self.cache.get_cache_filename(&specifier)?;
      let metadata = http_cache::Metadata::read(&cache_filename).ok()?;
      let maybe_content_type = metadata.headers.get("content-type").cloned();
      let maybe_headers = Some(&metadata.headers);
      let (_, maybe_charset) = map_content_type(&specifier, maybe_content_type);
      let content = Arc::new(get_source_from_bytes(bytes, maybe_charset).ok()?);
      Document::new(
        specifier.clone(),
        version,
        maybe_headers,
        content,
        self.get_maybe_resolver(),
      )
    };
    self.dirty = true;
    self.docs.insert(specifier, doc)
  }

  /// Iterate through the documents, building a map where the key is a unique
  /// document and the value is a set of specifiers that depend on that
  /// document.
  fn calculate_dependents(&mut self) {
    let mut dependents_map: HashMap<ModuleSpecifier, HashSet<ModuleSpecifier>> =
      HashMap::new();
    for (specifier, doc) in &self.docs {
      if let Some(Ok(module)) = &doc.maybe_module {
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
        if let Some((_, Some(Ok((dep, _))))) = &module.maybe_types_dependency {
          dependents_map
            .entry(dep.clone())
            .or_default()
            .insert(specifier.clone());
        }
      }
    }
    self.dependents_map = dependents_map;
  }

  fn calculate_version(&self, specifier: &ModuleSpecifier) -> Option<String> {
    let path = self.get_path(specifier)?;
    let metadata = fs::metadata(path).ok()?;
    if let Ok(modified) = metadata.modified() {
      if let Ok(n) = modified.duration_since(SystemTime::UNIX_EPOCH) {
        Some(n.as_millis().to_string())
      } else {
        Some("1".to_string())
      }
    } else {
      Some("1".to_string())
    }
  }

  fn change(
    &mut self,
    specifier: &ModuleSpecifier,
    version: i32,
    changes: Vec<lsp::TextDocumentContentChangeEvent>,
  ) -> Result<(), AnyError> {
    // this duplicates the .get_resolver() method, because there is no easy
    // way to avoid the double borrow of self that occurs here with getting the
    // mut doc out.
    let maybe_resolver = if self.maybe_jsx_resolver.is_some() {
      self.maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
    } else {
      self.maybe_import_map.as_ref().map(|im| im.as_resolver())
    };
    let doc = self.docs.get_mut(specifier).map_or_else(
      || {
        Err(custom_error(
          "NotFound",
          format!("The specifier \"{}\" was not found.", specifier),
        ))
      },
      Ok,
    )?;
    self.dirty = true;
    doc.change(version, changes, maybe_resolver)
  }

  fn close(&mut self, specifier: &ModuleSpecifier) -> Result<(), AnyError> {
    let doc = self.docs.get_mut(specifier).map_or_else(
      || {
        Err(custom_error(
          "NotFound",
          format!("The specifier \"{}\" was not found.", specifier),
        ))
      },
      Ok,
    )?;
    doc.close();
    self.dirty = true;
    Ok(())
  }

  fn contains_import(
    &mut self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> bool {
    let maybe_resolver = self.get_maybe_resolver();
    let maybe_specifier = if let Some(resolver) = maybe_resolver {
      resolver.resolve(specifier, referrer).ok()
    } else {
      deno_core::resolve_import(specifier, referrer.as_str()).ok()
    };
    if let Some(import_specifier) = maybe_specifier {
      self.contains_specifier(&import_specifier)
    } else {
      false
    }
  }

  fn contains_specifier(&mut self, specifier: &ModuleSpecifier) -> bool {
    let specifier = self
      .resolve_specifier(specifier)
      .unwrap_or_else(|| specifier.clone());
    if !self.is_valid(&specifier) {
      self.add(specifier.clone());
    }
    self.docs.contains_key(&specifier)
  }

  fn content(&mut self, specifier: &ModuleSpecifier) -> Option<Arc<String>> {
    self.get(specifier).map(|d| d.content())
  }

  fn dependencies(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<Vec<(String, deno_graph::Dependency)>> {
    let doc = self.get(specifier)?;
    let module = doc.maybe_module.as_ref()?.as_ref().ok()?;
    Some(
      module
        .dependencies
        .iter()
        .map(|(s, d)| (s.clone(), d.clone()))
        .collect(),
    )
  }

  fn dependents(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Vec<ModuleSpecifier> {
    if self.dirty {
      self.calculate_dependents();
      self.dirty = false;
    }
    let mut dependents = HashSet::new();
    if let Some(specifier) = self.resolve_specifier(specifier) {
      recurse_dependents(&specifier, &self.dependents_map, &mut dependents);
      dependents.into_iter().collect()
    } else {
      vec![]
    }
  }

  fn get(&mut self, specifier: &ModuleSpecifier) -> Option<&Document> {
    let specifier = self.resolve_specifier(specifier)?;
    if !self.is_valid(&specifier) {
      self.add(specifier.clone());
    }
    self.docs.get(&specifier)
  }

  fn get_maybe_dependency(
    &mut self,
    specifier: &ModuleSpecifier,
    position: &lsp::Position,
  ) -> Option<(String, deno_graph::Dependency, deno_graph::Range)> {
    let doc = self.get(specifier)?;
    let module = doc.maybe_module.as_ref()?.as_ref().ok()?;
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

  fn get_maybe_resolver(&self) -> Option<&dyn deno_graph::source::Resolver> {
    if self.maybe_jsx_resolver.is_some() {
      self.maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
    } else {
      self.maybe_import_map.as_ref().map(|im| im.as_resolver())
    }
  }

  fn get_maybe_types_for_dependency(
    &mut self,
    dependency: &deno_graph::Dependency,
  ) -> deno_graph::Resolved {
    let code_dep = dependency.maybe_code.as_ref()?;
    let (specifier, _) = code_dep.as_ref().ok()?;
    let doc = self.get(specifier)?;
    doc.maybe_types_dependency()
  }

  fn get_navigation_tree(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<Arc<tsc::NavigationTree>> {
    self
      .get(specifier)
      .map(|d| d.maybe_navigation_tree.clone())
      .flatten()
  }

  fn get_path(&self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    if specifier.scheme() == "file" {
      specifier.to_file_path().ok()
    } else {
      let path = self.cache.get_cache_filename(specifier)?;
      if path.is_file() {
        Some(path)
      } else {
        None
      }
    }
  }

  fn is_diagnosable(&mut self, specifier: &ModuleSpecifier) -> bool {
    if let Some(doc) = self.get(specifier) {
      doc.is_diagnosable()
    } else {
      false
    }
  }

  fn is_formattable(&mut self, specifier: &ModuleSpecifier) -> bool {
    // currently any document that is open in the language server is formattable
    self.is_open(specifier)
  }

  fn is_open(&mut self, specifier: &ModuleSpecifier) -> bool {
    let specifier = self
      .resolve_specifier(specifier)
      .unwrap_or_else(|| specifier.clone());
    // this does not use `self.get` since that lazily adds documents, and we
    // only care about documents already in the cache.
    if let Some(doc) = self.docs.get(&specifier) {
      doc.is_open()
    } else {
      false
    }
  }

  fn is_valid(&mut self, specifier: &ModuleSpecifier) -> bool {
    if self.is_open(specifier) {
      true
    } else if let Some(specifier) = self.resolve_specifier(specifier) {
      self.docs.get(&specifier).map(|d| d.version.clone())
        == self.calculate_version(&specifier)
    } else {
      // even though it isn't valid, it just can't exist, so we will say it is
      // valid
      true
    }
  }

  fn line_index(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<Arc<LineIndex>> {
    let specifier = self.resolve_specifier(specifier)?;
    self.docs.get(&specifier).map(|doc| doc.line_index.clone())
  }

  fn lsp_version(&self, specifier: &ModuleSpecifier) -> Option<i32> {
    self
      .docs
      .get(specifier)
      .map(|doc| doc.maybe_lsp_version)
      .flatten()
  }

  fn maybe_warning(&mut self, specifier: &ModuleSpecifier) -> Option<String> {
    self
      .get(specifier)
      .map(|d| d.maybe_warning.clone())
      .flatten()
  }

  fn open(
    &mut self,
    specifier: ModuleSpecifier,
    version: i32,
    language_id: LanguageId,
    content: Arc<String>,
  ) {
    let maybe_resolver = self.get_maybe_resolver();
    let document_data = Document::open(
      specifier.clone(),
      version,
      language_id,
      content,
      maybe_resolver,
    );
    self.docs.insert(specifier, document_data);
    self.dirty = true;
  }

  fn parsed_source(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<Result<deno_ast::ParsedSource, deno_graph::ModuleGraphError>> {
    self
      .get(specifier)
      .map(|doc| {
        doc.maybe_module.as_ref().map(|r| {
          r.as_ref()
            .map(|m| m.parsed_source.clone())
            .map_err(|err| err.clone())
        })
      })
      .flatten()
  }

  fn resolve(
    &mut self,
    specifiers: Vec<String>,
    referrer: &ModuleSpecifier,
  ) -> Option<Vec<Option<(ModuleSpecifier, MediaType)>>> {
    let doc = self.get(referrer)?;
    let mut results = Vec::new();
    if let Some(Ok(module)) = &doc.maybe_module {
      let dependencies = module.dependencies.clone();
      for specifier in specifiers {
        if specifier.starts_with("asset:") {
          if let Ok(specifier) = ModuleSpecifier::parse(&specifier) {
            let media_type = MediaType::from(&specifier);
            results.push(Some((specifier, media_type)));
          } else {
            results.push(None);
          }
        } else if let Some(dep) = dependencies.get(&specifier) {
          if let Some(Ok((specifier, _))) = &dep.maybe_type {
            results.push(self.resolve_dependency(specifier));
          } else if let Some(Ok((specifier, _))) = &dep.maybe_code {
            results.push(self.resolve_dependency(specifier));
          } else {
            results.push(None);
          }
        } else if let Some(Some(Ok((specifier, _)))) =
          self.resolve_imports_dependency(&specifier)
        {
          // clone here to avoid double borrow of self
          let specifier = specifier.clone();
          results.push(self.resolve_dependency(&specifier));
        } else {
          results.push(None);
        }
      }
    }
    Some(results)
  }

  fn resolve_dependency(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<(ModuleSpecifier, MediaType)> {
    let doc = self.get(specifier)?;
    let maybe_module =
      doc.maybe_module.as_ref().map(|r| r.as_ref().ok()).flatten();
    let maybe_types_dependency = maybe_module
      .map(|m| {
        m.maybe_types_dependency
          .as_ref()
          .map(|(_, o)| o.as_ref().map(|r| r.as_ref().ok()).flatten())
          .flatten()
      })
      .flatten()
      .cloned();
    if let Some((specifier, _)) = maybe_types_dependency {
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
    for module in self.imports.values() {
      let maybe_dep = module.dependencies.get(specifier);
      if maybe_dep.is_some() {
        return maybe_dep;
      }
    }
    None
  }

  fn resolve_remote_specifier(
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
        self.resolve_remote_specifier(&redirect, redirect_limit - 1)
      } else {
        Some(specifier.clone())
      }
    } else {
      None
    }
  }

  fn resolve_specifier(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    let scheme = specifier.scheme();
    if !SUPPORTED_SCHEMES.contains(&scheme) {
      return None;
    }

    if scheme == "data" || scheme == "blob" || scheme == "file" {
      Some(specifier.clone())
    } else if let Some(specifier) = self.redirects.get(specifier) {
      Some(specifier.clone())
    } else {
      let redirect = self.resolve_remote_specifier(specifier, 10)?;
      self.redirects.insert(specifier.clone(), redirect.clone());
      Some(redirect)
    }
  }

  fn set_location(&mut self, location: PathBuf) {
    // TODO update resolved dependencies?
    self.cache = HttpCache::new(&location);
    self.dirty = true;
  }

  fn set_navigation_tree(
    &mut self,
    specifier: &ModuleSpecifier,
    navigation_tree: Arc<tsc::NavigationTree>,
  ) -> Result<(), AnyError> {
    let doc = self.docs.get_mut(specifier).ok_or_else(|| {
      custom_error("NotFound", format!("Specifier not found {}", specifier))
    })?;
    doc.maybe_navigation_tree = Some(navigation_tree);
    Ok(())
  }

  fn specifiers(
    &self,
    open_only: bool,
    diagnosable_only: bool,
  ) -> Vec<ModuleSpecifier> {
    self
      .docs
      .iter()
      .filter_map(|(specifier, doc)| {
        let open = open_only && doc.is_open();
        let diagnosable = diagnosable_only && doc.is_diagnosable();
        if (!open_only || open) && (!diagnosable_only || diagnosable) {
          Some(specifier.clone())
        } else {
          None
        }
      })
      .collect()
  }

  fn text_info(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<SourceTextInfo> {
    self.get(specifier).map(|d| d.source.clone())
  }

  fn update_config(
    &mut self,
    maybe_import_map: Option<Arc<import_map::ImportMap>>,
    maybe_config_file: Option<&ConfigFile>,
  ) {
    // TODO(@kitsonk) update resolved dependencies?
    self.maybe_import_map = maybe_import_map.map(ImportMapResolver::new);
    self.maybe_jsx_resolver = maybe_config_file
      .map(|cf| {
        cf.to_maybe_jsx_import_source_module()
          .map(|im| JsxResolver::new(im, self.maybe_import_map.clone()))
      })
      .flatten();
    if let Some(Ok(Some(imports))) =
      maybe_config_file.map(|cf| cf.to_maybe_imports())
    {
      for (referrer, dependencies) in imports {
        let dependencies =
          dependencies.into_iter().map(|s| (s, None)).collect();
        let module = SyntheticModule::new(
          referrer.clone(),
          dependencies,
          self.get_maybe_resolver(),
        );
        self.imports.insert(referrer, module);
      }
    }
    self.dirty = true;
  }

  fn version(&mut self, specifier: &ModuleSpecifier) -> Option<String> {
    self.get(specifier).map(|d| {
      d.maybe_lsp_version
        .map_or_else(|| d.version.clone(), |v| v.to_string())
    })
  }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Documents(Arc<Mutex<Inner>>);

impl Documents {
  pub fn new(location: &Path) -> Self {
    Self(Arc::new(Mutex::new(Inner::new(location))))
  }

  /// Apply language server content changes to an open document.
  pub fn change(
    &self,
    specifier: &ModuleSpecifier,
    version: i32,
    changes: Vec<lsp::TextDocumentContentChangeEvent>,
  ) -> Result<(), AnyError> {
    self.0.lock().change(specifier, version, changes)
  }

  /// Close an open document, this essentially clears any editor state that is
  /// being held, and the document store will revert to the file system if
  /// information about the document is required.
  pub fn close(&self, specifier: &ModuleSpecifier) -> Result<(), AnyError> {
    self.0.lock().close(specifier)
  }

  /// Return `true` if the provided specifier can be resolved to a document,
  /// otherwise `false`.
  pub fn contains_import(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> bool {
    self.0.lock().contains_import(specifier, referrer)
  }

  /// Return `true` if the specifier can be resolved to a document.
  pub fn contains_specifier(&self, specifier: &ModuleSpecifier) -> bool {
    self.0.lock().contains_specifier(specifier)
  }

  /// If the specifier can be resolved to a document, return its current
  /// content, otherwise none.
  pub fn content(&self, specifier: &ModuleSpecifier) -> Option<Arc<String>> {
    self.0.lock().content(specifier)
  }

  /// Return an optional vector of dependencies for a given specifier.
  pub fn dependencies(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Vec<(String, deno_graph::Dependency)>> {
    self.0.lock().dependencies(specifier)
  }

  /// Return an array of specifiers, if any, that are dependent upon the
  /// supplied specifier. This is used to determine invalidation of diagnostics
  /// when a module has been changed.
  pub fn dependents(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Vec<ModuleSpecifier> {
    self.0.lock().dependents(specifier)
  }

  /// If the supplied position is within a dependency range, return the resolved
  /// string specifier for the dependency, the resolved dependency and the range
  /// in the source document of the specifier.
  pub fn get_maybe_dependency(
    &self,
    specifier: &ModuleSpecifier,
    position: &lsp::Position,
  ) -> Option<(String, deno_graph::Dependency, deno_graph::Range)> {
    self.0.lock().get_maybe_dependency(specifier, position)
  }

  /// For a given dependency, try to resolve the maybe_types_dependency for the
  /// dependency. This covers modules that assert their own types, like via the
  /// triple-slash reference, or the `X-TypeScript-Types` header.
  pub fn get_maybe_types_for_dependency(
    &self,
    dependency: &deno_graph::Dependency,
  ) -> deno_graph::Resolved {
    self.0.lock().get_maybe_types_for_dependency(dependency)
  }

  /// Get a reference to the navigation tree stored for a given specifier, if
  /// any.
  pub fn get_navigation_tree(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Arc<tsc::NavigationTree>> {
    self.0.lock().get_navigation_tree(specifier)
  }

  /// Indicates that a specifier is able to be diagnosed by the language server
  pub fn is_diagnosable(&self, specifier: &ModuleSpecifier) -> bool {
    self.0.lock().is_diagnosable(specifier)
  }

  /// Indicates that a specifier is formattable.
  pub fn is_formattable(&self, specifier: &ModuleSpecifier) -> bool {
    self.0.lock().is_formattable(specifier)
  }

  /// Return a reference to the line index for a given specifiers, if any.
  pub fn line_index(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Arc<LineIndex>> {
    self.0.lock().line_index(specifier)
  }

  /// Return the current language server client version for a given specifier,
  /// if any.
  pub fn lsp_version(&self, specifier: &ModuleSpecifier) -> Option<i32> {
    self.0.lock().lsp_version(specifier)
  }

  /// Return a warning header for a given specifier, if present.
  pub fn maybe_warning(&self, specifier: &ModuleSpecifier) -> Option<String> {
    self.0.lock().maybe_warning(specifier)
  }

  /// "Open" a document from the perspective of the editor, meaning that
  /// requests for information from the document will come from the in-memory
  /// representation received from the language server client, versus reading
  /// information from the disk.
  pub fn open(
    &self,
    specifier: ModuleSpecifier,
    version: i32,
    language_id: LanguageId,
    content: Arc<String>,
  ) {
    self.0.lock().open(specifier, version, language_id, content)
  }

  /// Return the parsed source or the module graph error for a given specifier.
  pub fn parsed_source(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Result<deno_ast::ParsedSource, deno_graph::ModuleGraphError>> {
    self.0.lock().parsed_source(specifier)
  }

  /// For a given set of string specifiers, resolve each one from the graph,
  /// for a given referrer. This is used to provide resolution information to
  /// tsc when type checking.
  pub fn resolve(
    &self,
    specifiers: Vec<String>,
    referrer: &ModuleSpecifier,
  ) -> Option<Vec<Option<(ModuleSpecifier, MediaType)>>> {
    self.0.lock().resolve(specifiers, referrer)
  }

  /// Update the location of the on disk cache for the document store.
  pub fn set_location(&self, location: PathBuf) {
    self.0.lock().set_location(location)
  }

  /// Set a navigation tree that is associated with the provided specifier.
  pub fn set_navigation_tree(
    &self,
    specifier: &ModuleSpecifier,
    navigation_tree: Arc<tsc::NavigationTree>,
  ) -> Result<(), AnyError> {
    self
      .0
      .lock()
      .set_navigation_tree(specifier, navigation_tree)
  }

  /// Return a vector of specifiers that are contained in the document store,
  /// where `open_only` flag would provide only those documents currently open
  /// in the editor and `diagnosable_only` would provide only those documents
  /// that the language server can provide diagnostics for.
  pub fn specifiers(
    &self,
    open_only: bool,
    diagnosable_only: bool,
  ) -> Vec<ModuleSpecifier> {
    self.0.lock().specifiers(open_only, diagnosable_only)
  }

  /// Return the current text info for a given specifier.
  pub fn text_info(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<SourceTextInfo> {
    self.0.lock().text_info(specifier)
  }

  pub fn update_config(
    &self,
    maybe_import_map: Option<Arc<import_map::ImportMap>>,
    maybe_config_file: Option<&ConfigFile>,
  ) {
    self
      .0
      .lock()
      .update_config(maybe_import_map, maybe_config_file)
  }

  /// Return the version of a document in the document cache.
  pub fn version(&self, specifier: &ModuleSpecifier) -> Option<String> {
    self.0.lock().version(specifier)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  fn setup() -> (Documents, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let location = temp_dir.path().join("deps");
    let documents = Documents::new(&location);
    (documents, location)
  }

  #[test]
  fn test_documents_open() {
    let (documents, _) = setup();
    let specifier = ModuleSpecifier::parse("file:///a.ts").unwrap();
    let content = Arc::new(
      r#"import * as b from "./b.ts";
console.log(b);
"#
      .to_string(),
    );
    documents.open(
      specifier.clone(),
      1,
      "javascript".parse().unwrap(),
      content,
    );
    assert!(documents.is_formattable(&specifier));
    assert!(documents.is_diagnosable(&specifier));
    assert!(documents.line_index(&specifier).is_some());
  }

  #[test]
  fn test_documents_change() {
    let (documents, _) = setup();
    let specifier = ModuleSpecifier::parse("file:///a.ts").unwrap();
    let content = Arc::new(
      r#"import * as b from "./b.ts";
console.log(b);
"#
      .to_string(),
    );
    documents.open(
      specifier.clone(),
      1,
      "javascript".parse().unwrap(),
      content,
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
      documents.content(&specifier).unwrap().as_str(),
      r#"import * as b from "./b.ts";
console.log(b, "hello deno");
"#
    );
  }
}
