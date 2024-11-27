use std::collections::HashMap;

use deno_ast::{MediaType, ParsedSource};
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
  pub ast: Option<ParsedSource>,
  pub dependencies: Vec<BundleDep>,
}

#[derive(Debug)]
pub enum BundleModule {
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
  modules: HashMap<usize, BundleModule>,
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

  pub fn get_specifier(&self, id: usize) -> Option<Url> {
    if let Some(module) = self.modules.get(&id) {
      match module {
        BundleModule::Js(m) => Some(m.specifier.clone()),
        BundleModule::Json(m) => Some(m.specifier.clone()),
        BundleModule::Wasm(m) => Some(m.specifier.clone()),
        BundleModule::Node(_) => None, // FIXME
        BundleModule::External(external_module) => todo!(),
      }
    } else {
      None
    }
  }

  pub fn get(&self, url: &Url) -> Option<&BundleModule> {
    if let Some(id) = self.url_to_id.get(&url) {
      return self.modules.get(&id);
    }

    None
  }

  pub fn insert(&mut self, specifier: Url, module: BundleModule) -> usize {
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
    if let Some(BundleModule::Js(module)) = self.modules.get_mut(&id) {
      module.dependencies.push(dep)
    }
  }
}
