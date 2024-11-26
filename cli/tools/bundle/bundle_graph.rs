use std::collections::HashMap;

use deno_ast::MediaType;
use deno_core::url::Url;
use deno_graph::{ExternalModule, JsonModule, WasmModule};

#[derive(Debug)]
pub struct BundleDep {
  pub id: usize,
  pub raw: String,
  pub is_dyanmic: bool,
}

#[derive(Debug)]
pub struct BundleJsModule {
  pub specifier: Url,
  pub media_type: MediaType,
  pub source: String,
  pub dependencies: Vec<BundleDep>,
}

#[derive(Debug)]
pub enum BundleMod {
  Js(BundleJsModule),
  Json(JsonModule),
  Wasm(WasmModule),
  Node(String),
  External(ExternalModule),
}

#[derive(Debug)]
pub struct BundleGraph {
  id: usize,
  url_to_id: HashMap<Url, usize>,
  modules: HashMap<usize, BundleMod>,
}

/// The bundle graph only contains fully resolved modules.
impl BundleGraph {
  pub fn new() -> Self {
    Self {
      id: 0,
      url_to_id: HashMap::new(),
      modules: HashMap::new(),
    }
  }

  pub fn insert(&mut self, specifier: Url, module: BundleMod) -> usize {
    let id = self.register(specifier);
    self.modules.insert(id, module);
    id
  }

  pub fn register(&mut self, specifier: Url) -> usize {
    if let Some(id) = self.url_to_id.get(&specifier) {
      *id
    } else {
      let id = self.id;
      self.id += 1;
      self.url_to_id.insert(specifier, id);
      id
    }
  }

  pub fn add_dependency(&mut self, id: usize, dep: BundleDep) {
    if let Some(BundleMod::Js(module)) = self.modules.get_mut(&id) {
      module.dependencies.push(dep)
    }
  }
}
