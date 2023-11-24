// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::super::cache::calculate_fs_version;
use super::super::cache::calculate_fs_version_at_path;
use super::Document;
use crate::cache::HttpCache;
use crate::file_fetcher::get_source_from_bytes;
use crate::file_fetcher::get_source_from_data_url;
use crate::file_fetcher::map_content_type;
use crate::util::path::specifier_to_file_path;
use crate::util::text_encoding;
use deno_ast::SourceTextInfo;
use deno_core::ModuleSpecifier;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct FileSystemDocuments {
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

  /// Removes a document from the cache without marking the cache as dirty.
  pub fn remove(&mut self, specifier: &ModuleSpecifier) -> Option<Document> {
    self.docs.remove(specifier)
  }

  /// Get an iterator over the file system documents.
  pub fn values(&self) -> impl Iterator<Item = &Document> {
    self.docs.values()
  }

  /// Get an iterator over the file system documents.
  pub fn iter(&self) -> impl Iterator<Item = (&ModuleSpecifier, &Document)> {
    self.docs.iter()
  }

  /// Insert a known document into the cache.
  pub fn insert(&mut self, specifier: ModuleSpecifier, doc: Document) {
    self.docs.insert(specifier, doc);
    self.dirty = true;
  }

  pub fn get_mut(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<&mut Document> {
    self.docs.get_mut(specifier)
  }

  pub fn keys(&self) -> impl Iterator<Item = &ModuleSpecifier> {
    self.docs.keys()
  }

  pub fn contains_specifier(&self, specifier: &ModuleSpecifier) -> bool {
    self.docs.contains_key(specifier)
  }

  /// Update an existing document to have a new resolver
  pub fn update_resolver_for_document(
    &mut self,
    specifier: &ModuleSpecifier,
    resolver: &dyn deno_graph::source::Resolver,
    npm_resolver: &dyn deno_graph::source::NpmResolver,
  ) {
    let Some(doc) = self.docs.get_mut(&specifier) else {
      return;
    };
    let Some(new_doc) = doc.maybe_with_new_resolver(resolver, npm_resolver)
    else {
      return;
    };
    *doc = new_doc;
    self.dirty = true;
  }

  /// Update all documents to have a new resolver
  pub fn update_resolver_for_all_documents(
    &mut self,
    resolver: &dyn deno_graph::source::Resolver,
    npm_resolver: &dyn deno_graph::source::NpmResolver,
  ) {
    self.docs.values_mut().for_each(|doc| {
      let Some(new_doc) = doc.maybe_with_new_resolver(resolver, npm_resolver)
      else {
        return;
      };

      *doc = new_doc;
    });
    self.dirty = true;
  }

  pub fn is_dirty(&self) -> bool {
    self.dirty
  }

  pub fn reset_dirty(&mut self) {
    self.dirty = false;
  }

  /// Adds or updates a document by reading the document from the file system
  /// returning the document.
  pub fn refresh_document(
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
      let content: String = get_source_from_bytes(bytes, maybe_charset).ok()?;
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
