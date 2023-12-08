// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::super::cache::calculate_fs_version;
use super::super::text::LineIndex;
use super::super::tsc;
use super::file_like_to_file_specifier;
use super::LanguageId;
use crate::cache::HttpCache;
use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::ModuleSpecifier;
use deno_graph::Resolution;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use tower_lsp::lsp_types as lsp;

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

#[derive(Debug, Default)]
struct DocumentDependencies {
  deps: IndexMap<String, deno_graph::Dependency>,
  maybe_types_dependency: Option<deno_graph::TypesDependency>,
}

impl DocumentDependencies {
  fn from_maybe_module(maybe_module: &Option<ModuleResult>) -> Self {
    if let Some(Ok(module)) = &maybe_module {
      Self::from_module(module)
    } else {
      Self::default()
    }
  }

  fn from_module(module: &deno_graph::EsmModule) -> Self {
    let mut deps = Self {
      deps: module.dependencies.clone(),
      maybe_types_dependency: module.maybe_types_dependency.clone(),
    };
    if file_like_to_file_specifier(&module.specifier).is_some() {
      for (_, dep) in &mut deps.deps {
        if let Resolution::Ok(resolved) = &mut dep.maybe_code {
          if let Some(specifier) =
            file_like_to_file_specifier(&resolved.specifier)
          {
            resolved.specifier = specifier;
          }
        }
        if let Resolution::Ok(resolved) = &mut dep.maybe_type {
          if let Some(specifier) =
            file_like_to_file_specifier(&resolved.specifier)
          {
            resolved.specifier = specifier;
          }
        }
      }
      if let Some(dep) = &mut deps.maybe_types_dependency {
        if let Resolution::Ok(resolved) = &mut dep.dependency {
          if let Some(specifier) =
            file_like_to_file_specifier(&resolved.specifier)
          {
            resolved.specifier = specifier;
          }
        }
      }
    }
    deps
  }
}

type ModuleResult = Result<deno_graph::EsmModule, deno_graph::ModuleGraphError>;
type ParsedSourceResult = Result<ParsedSource, deno_ast::Diagnostic>;

#[derive(Debug)]
struct Notebook {
  // /// The specifiers of all the cells in the notebook.
  // cells: Vec<ModuleSpecifier>,
}

#[derive(Debug)]
struct DocumentInner {
  /// Contains the last-known-good set of dependencies from parsing the module.
  dependencies: Arc<DocumentDependencies>,
  fs_version: String,
  line_index: Arc<LineIndex>,
  maybe_headers: Option<HashMap<String, String>>,
  /// Contains the language ID if known.
  maybe_language_id: Option<LanguageId>,
  /// Contains the lsp version if known.
  maybe_lsp_version: Option<i32>,
  maybe_module: Option<ModuleResult>,
  // this is a lazily constructed value based on the state of the document,
  // so having a mutex to hold it is ok
  maybe_navigation_tree: Mutex<Option<Arc<tsc::NavigationTree>>>,
  maybe_parsed_source: Option<ParsedSourceResult>,
  specifier: ModuleSpecifier,
  text_info: SourceTextInfo,
  /// The associated notebook if this document is a notebook cell.
  maybe_notebook: Option<Arc<Notebook>>,
}

#[derive(Debug, Clone)]
pub struct Document(Arc<DocumentInner>);

impl Document {
  /// Creates a new document from a specifier and its associated text.
  ///
  /// Is used to create a document when loading a file from disk.
  pub fn new(
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
      maybe_notebook: None,
    }))
  }

  /// Used to change the resolver configuration for a document, if it changed globally.
  pub fn maybe_with_new_resolver(
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
      maybe_notebook: self.0.maybe_notebook.clone(),
    })))
  }

  /// Creates a new document from a specifier and its associated text.
  ///
  /// Used when the document is opened via the language server client.
  pub fn open(
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
      maybe_notebook: None,
    }))
  }

  /// Apply a change to a document
  pub fn with_change(
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
      maybe_notebook: self.0.maybe_notebook.clone(),
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
      maybe_notebook: self.0.maybe_notebook.clone(),
    }))
  }

  pub fn specifier(&self) -> &ModuleSpecifier {
    &self.0.specifier
  }

  pub fn text(&self) -> Arc<str> {
    self.0.text_info.text()
  }

  pub fn text_info(&self) -> SourceTextInfo {
    self.0.text_info.clone()
  }

  pub fn line_index(&self) -> Arc<LineIndex> {
    self.0.line_index.clone()
  }

  pub fn fs_version(&self) -> &str {
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

  /// Return the language ID if known.
  pub fn maybe_language_id(&self) -> Option<LanguageId> {
    self.0.maybe_language_id
  }

  /// Returns the current language server client version if any.
  pub fn maybe_lsp_version(&self) -> Option<i32> {
    self.0.maybe_lsp_version
  }

  pub fn maybe_esm_module(&self) -> Option<&ModuleResult> {
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
      deno_graph::GraphKind::All,
      specifier,
      maybe_headers,
      parsed_source,
      Some(resolver),
      Some(npm_resolver),
    )),
    Err(err) => Err(deno_graph::ModuleGraphError::ModuleError(
      deno_graph::ModuleError::ParseErr(specifier.clone(), err.clone()),
    )),
  }
}
