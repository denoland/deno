use std::collections::{HashMap, HashSet};

use deno_core::url::Url;
use indexmap::IndexSet;

use crate::args::{BundleFlags, BundlePlatform};

use super::bundle_graph::{BundleGraph, BundleModule};

pub fn assign_chunks(
  bundle_flags: &BundleFlags,
  chunk_graph: &mut ChunkGraph,
  graph: &BundleGraph,
  url: &Url,
  parent_chunk_id: Option<usize>,
  is_dynamic: bool,
) {
  let module = graph.get(url).unwrap();

  match &module {
    &BundleModule::Js(js_module) => {
      let chunk_id = chunk_graph.assign_specifier_to_chunk(
        url,
        parent_chunk_id,
        ChunkKind::Js,
        is_dynamic,
      );

      for dep in &js_module.dependencies {
        if let Some(specifier) = graph.get_specifier(dep.id) {
          assign_chunks(
            bundle_flags,
            chunk_graph,
            graph,
            &specifier,
            Some(chunk_id),
            dep.is_dyanmic,
          );
        }
      }
    }
    BundleModule::Json(json_module) => {
      chunk_graph.assign_specifier_to_chunk(
        url,
        parent_chunk_id,
        ChunkKind::Js,
        is_dynamic,
      );
    }
    BundleModule::Wasm(wasm_module) => {
      chunk_graph.assign_specifier_to_chunk(
        url,
        parent_chunk_id,
        ChunkKind::Asset("wasm".to_string()),
        true,
      );
    }
    BundleModule::Node(built_in_node_module) => {
      if let BundlePlatform::Browser = bundle_flags.platform {
        // TODO: Show where it was imported from
        log::log!(
          log::Level::Error,
          "Imported Node internal module '{}' which will fail in browsers.",
          built_in_node_module
        );
      }
    }
    BundleModule::External(external_module) => todo!(),
  }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ChunkKind {
  Asset(String),
  Js,
}

#[derive(Debug)]
pub struct Chunk {
  pub id: usize,
  pub name: String,
  pub kind: ChunkKind,
  pub parent_ids: HashSet<usize>,
  pub children: Vec<usize>, // TODO: IndexSet?
  pub specifiers: IndexSet<Url>,
}

#[derive(Debug)]
pub struct ChunkGraph {
  pub id: usize,
  pub chunks: HashMap<usize, Chunk>,
  pub root_chunks: HashSet<usize>,
  pub specifier_to_chunks: HashMap<Url, Vec<usize>>,
}

impl ChunkGraph {
  pub fn new() -> Self {
    Self {
      id: 0,
      chunks: HashMap::new(),
      root_chunks: HashSet::new(),
      specifier_to_chunks: HashMap::new(),
    }
  }

  fn get_or_create_chunk(
    &mut self,
    url: &Url,
    parent_chunk_id: Option<usize>,
    kind: ChunkKind,
    is_dynamic: bool,
  ) -> usize {
    if let Some(parent_chunk_id) = parent_chunk_id {
      if !is_dynamic {
        return parent_chunk_id;
      }
    }

    let name = if let Ok(f) = url.to_file_path() {
      if let Some(name) = f.file_stem() {
        name.to_string_lossy().to_string()
      } else {
        format!("chunk_{}", self.id)
      }
    } else {
      format!("chunk_{}", self.id)
    };

    let ext = match &kind {
      ChunkKind::Asset(ext) => ext,
      ChunkKind::Js => "js",
    };

    let full_name = format!("{}.{}", name, ext);

    self.new_chunk(full_name, parent_chunk_id, kind)
  }

  pub fn assign_specifier_to_chunk(
    &mut self,
    url: &Url,
    parent_chunk_id: Option<usize>,
    kind: ChunkKind,
    is_dynamic: bool,
  ) -> usize {
    let chunk_id =
      self.get_or_create_chunk(&url, parent_chunk_id, kind, is_dynamic);

    if let Some(value) = self.specifier_to_chunks.get_mut(&url) {
      value.push(chunk_id)
    } else {
      let value = vec![chunk_id];
      self.specifier_to_chunks.insert(url.clone(), value);
    }

    if let Some(chunk) = self.chunks.get_mut(&chunk_id) {
      chunk.specifiers.insert(url.clone());
    }

    chunk_id
  }

  fn new_chunk(
    &mut self,
    name: String,
    parent_id: Option<usize>,
    kind: ChunkKind,
  ) -> usize {
    let id = self.id;
    self.id += 1;

    let mut parent_ids = HashSet::new();
    if let Some(parent_id) = parent_id {
      parent_ids.insert(parent_id);
    } else {
      self.root_chunks.insert(id);
    }

    let chunk = Chunk {
      id,
      name,
      kind,
      parent_ids,
      children: vec![],
      specifiers: IndexSet::new(),
    };

    self.chunks.insert(id, chunk);
    id
  }
}
