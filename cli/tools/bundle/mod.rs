use std::{
  collections::{HashMap, HashSet},
  fs,
  path::Path,
  sync::Arc,
};

use deno_core::{error::AnyError, url::Url};
use deno_graph::{GraphKind, Module, ModuleGraph};
use deno_runtime::colors;

use crate::{
  args::{BundleFlags, BundlePlatform, Flags},
  factory::CliFactory,
  graph_util::CreateGraphOptions,
  util::{fs::collect_specifiers, path::matches_pattern_or_exact_path},
};

pub async fn bundle(
  flags: Arc<Flags>,
  bundle_flags: BundleFlags,
) -> Result<(), AnyError> {
  // FIXME: Permissions
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let deno_dir = factory.deno_dir()?;
  let http_client = factory.http_client_provider();

  // TODO: Ensure that dependencies are installed

  let file_patterns = bundle_flags
    .files
    .as_file_patterns(cli_options.initial_cwd())?;
  let files = collect_specifiers(file_patterns, None, |entry| {
    if let Some(include) = &entry.patterns.include {
      // allow someone to explicitly specify a path
      matches_pattern_or_exact_path(include, entry.path)
    } else {
      false
    }
  })?;

  let module_graph_creator = factory.module_graph_creator().await?;

  let graph = module_graph_creator
    .create_graph_with_options(CreateGraphOptions {
      graph_kind: GraphKind::CodeOnly,
      roots: files.clone(),
      is_dynamic: false,
      loader: None,
    })
    .await?;

  graph.valid()?;

  let mut chunk_graph = ChunkGraph::new();
  for file in files {
    let chunk_id = chunk_graph.new_chunk(None);
    if let Some(module) = graph.get(&file) {
      assign_chunks(&bundle_flags, &mut chunk_graph, &graph, module, chunk_id);
    }
  }

  // Hoist shared modules into common parent chunk that is not a root chunk
  //for c

  // Ensure output directory exists
  let out_dir = Path::new(&bundle_flags.out_dir);
  fs::create_dir_all(out_dir)?;

  // Write out chunks
  // TODO: Walk topo for chunk hashes
  for (_id, chunk) in &chunk_graph.chunks {
    //chunk
    let mut source = String::new();

    for spec in &chunk.specifiers {
      if let Some(module) = graph.get(&spec) {
        //
        source.push_str(&format!("// {}\n", spec.to_string()));
        if let Some(contents) = &module.source() {
          source.push_str(contents);
        }
      }
    }

    let out_path = out_dir.join("out.js");
    fs::write(&out_path, source).unwrap();
    log::log!(
      log::Level::Info,
      "{} {}",
      colors::green("Filename"),
      colors::green("Size")
    );
    log::log!(
      log::Level::Info,
      "  {} {}",
      out_path.to_string_lossy(),
      colors::cyan(0)
    );
    log::log!(log::Level::Info, "");
  }

  Ok(())
}

fn assign_chunks(
  bundle_flags: &BundleFlags,
  chunk_graph: &mut ChunkGraph,
  graph: &ModuleGraph,
  module: &Module,
  chunk_id: usize,
) {
  match module {
    Module::Js(js_module) => {
      chunk_graph.assign_specifier(js_module.specifier.clone(), chunk_id);

      for (value, dep) in &js_module.dependencies {
        let url = Url::parse(value).unwrap();
        let chunk_id = if dep.is_dynamic {
          chunk_graph.new_chunk(Some(chunk_id))
        } else {
          chunk_id
        };

        if let Some(module) = graph.get(&url) {
          assign_chunks(bundle_flags, chunk_graph, graph, module, chunk_id);
        }
      }
    }
    Module::Json(json_module) => {
      chunk_graph.assign_specifier(json_module.specifier.clone(), chunk_id);
    }
    Module::Wasm(wasm_module) => {
      let chunk_id = chunk_graph.new_chunk(Some(chunk_id));
      chunk_graph.assign_specifier(wasm_module.specifier.clone(), chunk_id);
    }
    Module::Npm(npm_module) => todo!(),
    Module::Node(built_in_node_module) => {
      if let BundlePlatform::Browser = bundle_flags.platform {
        // TODO: Show where it was imported from
        log::log!(
          log::Level::Error,
          "Imported Node internal module '{}' which will fail in browsers.",
          built_in_node_module.specifier.to_string()
        );
      }
    }
    Module::External(external_module) => todo!(),
  }
}

#[derive(Debug)]
struct Chunk {
  id: usize,
  parent_ids: HashSet<usize>,
  children: Vec<usize>, // TODO: IndexSet?
  specifiers: HashSet<Url>,
}

#[derive(Debug)]
struct ChunkGraph {
  pub id: usize,
  pub chunks: HashMap<usize, Chunk>,
  pub root_chunks: HashSet<usize>,
  pub specifier_to_chunks: HashMap<Url, Vec<usize>>,
}

impl ChunkGraph {
  fn new() -> Self {
    Self {
      id: 0,
      chunks: HashMap::new(),
      root_chunks: HashSet::new(),
      specifier_to_chunks: HashMap::new(),
    }
  }

  fn assign_specifier(&mut self, url: Url, chunk_id: usize) {
    if let Some(value) = self.specifier_to_chunks.get_mut(&url) {
      value.push(chunk_id)
    } else {
      let value = vec![chunk_id];
      self.specifier_to_chunks.insert(url.clone(), value);
    }

    if let Some(chunk) = self.chunks.get_mut(&chunk_id) {
      chunk.specifiers.insert(url);
    }
  }

  fn new_chunk(&mut self, parent_id: Option<usize>) -> usize {
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
      parent_ids,
      children: vec![],
      specifiers: HashSet::new(),
    };

    self.chunks.insert(id, chunk);
    id
  }
}
