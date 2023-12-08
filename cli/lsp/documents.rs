// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
mod asset_or_document;
mod document;
mod file_like_to_file_specifier;
mod file_system_documents;
mod language_id;
mod open_documents_graph_loader;
mod preload_document_finder;
mod to_lsp_range;
pub use file_like_to_file_specifier::file_like_to_file_specifier;
use preload_document_finder::{
  PreloadDocumentFinder, PreloadDocumentFinderOptions,
};
pub use to_lsp_range::to_lsp_range;
mod specifier_resolver;
pub use asset_or_document::AssetOrDocument;
pub use document::Document;
use file_system_documents::FileSystemDocuments;
pub use language_id::LanguageId;
pub use open_documents_graph_loader::OpenDocumentsGraphLoader;
use specifier_resolver::SpecifierResolver;

use super::language_server::StateNpmSnapshot;
use super::tsc;

use crate::args::package_json;
use crate::args::package_json::PackageJsonDeps;
use crate::args::ConfigFile;
use crate::args::JsxImportSourceConfig;
use crate::cache::FastInsecureHasher;
use crate::cache::HttpCache;
use crate::lsp::logging::lsp_warn;
use crate::npm::CliNpmResolver;
use crate::resolver::CliGraphResolver;
use crate::resolver::CliGraphResolverOptions;
use crate::util::path::specifier_to_file_path;

use deno_ast::MediaType;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
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
use package_json::PackageJsonDepsProvider;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tower_lsp::lsp_types as lsp;

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
  specifier_resolver: Arc<SpecifierResolver>,
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
      })),
      npm_specifier_reqs: Default::default(),
      has_injected_types_node_package: false,
      specifier_resolver: Arc::new(SpecifierResolver::new(cache)),
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
    file_system_docs.remove(&specifier);
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
        file_system_docs.remove(specifier)
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
      file_system_docs.remove(specifier)
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
      file_system_docs.insert(specifier.clone(), document);
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
      .resolve(specifier, referrer, ResolutionMode::Types)
      .ok();
    if let Some(import_specifier) = maybe_specifier {
      self.exists(&import_specifier)
    } else {
      false
    }
  }

  pub fn resolve_redirected(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<ModuleSpecifier> {
    self.specifier_resolver.resolve(specifier)
  }

  /// Return `true` if the specifier can be resolved to a document.
  pub fn exists(&self, specifier: &ModuleSpecifier) -> bool {
    let specifier = self.specifier_resolver.resolve(specifier);
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
    if let Some(specifier) = self.specifier_resolver.resolve(specifier) {
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
    let specifier = self.specifier_resolver.resolve(original_specifier)?;
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
          .chain(file_system_docs.values())
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
    let dependencies = referrer_doc.dependencies();
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
      } else if let Some(dep) = dependencies.and_then(|d| d.get(&specifier)) {
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
    self.specifier_resolver = Arc::new(SpecifierResolver::new(cache));
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
      if let Some(doc) = file_system_docs.get_mut(specifier) {
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
    );
    let deps_provider =
      Arc::new(PackageJsonDepsProvider::new(maybe_package_json_deps));
    self.resolver = Arc::new(CliGraphResolver::new(CliGraphResolverOptions {
      fs: Arc::new(RealFs),
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
        .map(|config| {
          config
            .json
            .unstable
            .contains(&"bare-node-builtins".to_string())
        })
        .unwrap_or(false),
    }));
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
    let mut file_system_docs = self.file_system_docs.lock();
    if document_preload_limit > 0 {
      let mut not_found_docs =
        file_system_docs.keys().cloned().collect::<HashSet<_>>();
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
          && !file_system_docs.contains_specifier(&specifier)
        {
          file_system_docs.refresh_document(
            &self.cache,
            resolver,
            &specifier,
            npm_resolver,
          );
        } else {
          // update the existing entry to have the new resolver
          file_system_docs.update_resolver_for_document(
            &specifier,
            resolver,
            npm_resolver,
          );
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
          file_system_docs.update_resolver_for_document(
            &uri,
            resolver,
            npm_resolver,
          );
        }
      } else {
        // clean up and remove any documents that weren't found
        for uri in not_found_docs {
          file_system_docs.remove(&uri);
        }
      }
    } else {
      // This log statement is used in the tests to ensure preloading doesn't
      // happen, which is not useful in the repl and could be very expensive
      // if the repl is launched from a directory with a lot of descendants.
      log::debug!("Skipping document preload.");

      // just update to use the new resolver
      file_system_docs
        .update_resolver_for_all_documents(resolver, npm_resolver);
    }
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
    if !file_system_docs.is_dirty() && !self.dirty {
      return;
    }

    let mut doc_analyzer = DocAnalyzer::default();
    // favor documents that are open in case a document exists in both collections
    let documents = file_system_docs.iter().chain(self.open_docs.iter());
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
    file_system_docs.reset_dirty();
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::cache::GlobalHttpCache;
  use crate::cache::RealDenoCacheEnv;
  use import_map::ImportMap;
  use pretty_assertions::assert_eq;
  use std::fs;
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
      &*documents.get(&specifier).unwrap().text(),
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
}
