use std::{
  cmp::Ordering,
  collections::{HashMap, HashSet},
  fs,
  io::Write,
  path::{Path, PathBuf},
  sync::Arc,
};

use bundle_graph::{BundleDep, BundleGraph, BundleJsModule, BundleMod};
use deno_core::{anyhow::Context, error::AnyError, url::Url};
use deno_graph::{GraphKind, Module, ModuleGraph, NpmModule, Resolution};
use deno_runtime::{colors, deno_node::NodeResolver};
use deno_semver::package;
use flate2::{write::ZlibEncoder, Compression};
use indexmap::IndexSet;
use node_resolver::{NodeModuleKind, NodeResolutionMode};

use crate::{
  args::{BundleFlags, BundlePlatform, Flags},
  factory::CliFactory,
  graph_util::CreateGraphOptions,
  npm::CliNpmResolver,
  resolver::CjsTracker,
  util::{fs::collect_specifiers, path::matches_pattern_or_exact_path},
};

mod bundle_graph;

#[derive(Debug)]
struct BundleChunkStat {
  name: PathBuf,
  size: usize,
  gzip: usize,
  brotli: usize,
}

pub async fn bundle(
  flags: Arc<Flags>,
  bundle_flags: BundleFlags,
) -> Result<(), AnyError> {
  // FIXME: Permissions
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let npm_resolver = factory.npm_resolver().await?;
  let node_resolver = factory.node_resolver().await?;
  let cjs_tracker = factory.cjs_tracker()?;

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

  let mut bundle_graph = BundleGraph::new();

  let mut id = 0;
  let mut all_modules: HashMap<Url, Module> = HashMap::new();
  let mut module_to_id: HashMap<Url, usize> = HashMap::new();
  let mut id_to_module: HashMap<usize, Url> = HashMap::new();

  let mut npm_modules: IndexSet<Url> = IndexSet::new();
  let mut seen: IndexSet<Url> = IndexSet::new();

  fn resolve_npm_module(
    module: &NpmModule,
    npm_resolver: &Arc<dyn CliNpmResolver>,
    node_resolver: &Arc<NodeResolver>,
  ) -> Url {
    let nv = module.nv_reference.nv();
    let managed = npm_resolver.as_managed().unwrap();
    let package_folder =
      managed.resolve_pkg_folder_from_deno_module(nv).unwrap();

    let resolved = node_resolver
      .resolve_package_subpath_from_deno_module(
        &package_folder,
        module.nv_reference.sub_path(),
        None,                // FIXME
        NodeModuleKind::Esm, // FIXME
        NodeResolutionMode::Execution,
      )
      .with_context(|| format!("Could not resolve '{}'.", module.nv_reference))
      .unwrap();

    resolved
  }

  let mut pending_npm_dep_links: HashMap<usize, Url> = HashMap::new();

  // Hack: Create sub graphs for every npm module we encounter and
  // expand npm specifiers to the actual file
  for module in graph.modules() {
    let url = module.specifier();
    seen.insert(url.clone());

    let current_id = id;
    module_to_id.insert(url.clone(), current_id);
    id_to_module.insert(current_id, url.clone());

    id += 1;

    match module {
      Module::Npm(module) => {
        let resolved = resolve_npm_module(&module, npm_resolver, node_resolver);
        npm_modules.insert(resolved.clone());
      }
      Module::Js(js_module) => {
        let id = bundle_graph.insert(
          url.clone(),
          BundleMod::Js(BundleJsModule {
            specifier: url.clone(),
            media_type: js_module.media_type,
            source: js_module.source.to_string(),
            dependencies: vec![],
          }),
        );

        for (raw, dep) in &js_module.dependencies {
          if let Some(code) = dep.get_code() {
            if code.scheme() == "npm" {
              pending_npm_dep_links.insert(id, code.clone());
            } else {
              let dep_id = bundle_graph.register(code.clone());
              bundle_graph.add_dependency(
                id,
                BundleDep {
                  id: dep_id,
                  raw: raw.to_string(),
                  is_dyanmic: dep.is_dynamic,
                },
              );
            }
          }
          eprintln!("JS dep {} {:#?}", raw, dep);
        }
      }
      Module::Json(json_module) => {
        bundle_graph.insert(url.clone(), BundleMod::Json(json_module.clone()));
      }
      Module::Wasm(wasm_module) => {
        bundle_graph.insert(url.clone(), BundleMod::Wasm(wasm_module.clone()));
      }
      Module::Node(built_in_node_module) => {
        bundle_graph.insert(
          url.clone(),
          BundleMod::Node(built_in_node_module.module_name.to_string()),
        );
      }
      Module::External(external_module) => todo!(),
    }
  }

  let npm_modules_vec =
    npm_modules.iter().map(|u| u.clone()).collect::<Vec<_>>();

  eprintln!("npm vec: {:#?}", npm_modules_vec);
  while let Some(url) = npm_modules.pop() {
    let npm_graph = module_graph_creator
      .create_graph_with_options(CreateGraphOptions {
        graph_kind: GraphKind::CodeOnly,
        roots: vec![url.clone()],
        is_dynamic: false,
        loader: None,
      })
      .await?;

    for module in npm_graph.modules() {
      if seen.contains(module.specifier()) {
        continue;
      }

      match module {
        Module::Npm(module) => {
          let resolved =
            resolve_npm_module(&module, npm_resolver, node_resolver);
          npm_modules.insert(resolved.clone());
        }
        _ => {
          all_modules.insert(url.clone(), module.clone());
        }
      }
    }

    eprintln!("RES {:#?}", all_modules);
  }

  let mut chunk_graph = ChunkGraph::new();
  for file in files {
    assign_chunks(
      &bundle_flags,
      &mut chunk_graph,
      &all_modules,
      npm_resolver,
      node_resolver,
      cjs_tracker,
      &file,
      None,
      true,
    );
  }

  // Hoist shared modules into common parent chunk that is not a root chunk
  //for c

  // Ensure output directory exists
  let out_dir = Path::new(&bundle_flags.out_dir);
  fs::create_dir_all(out_dir)?;

  let mut stats: Vec<BundleChunkStat> = vec![];
  let mut cols = (8, 4, 4, 6);

  // Write out chunks
  // TODO: Walk topo for chunk hashes
  for (_id, chunk) in &chunk_graph.chunks {
    //chunk
    let mut source = String::new();

    for spec in chunk.specifiers.iter().rev() {
      if let Some(module) = graph.get(&spec) {
        // FIXME: don't print module urls by default
        source.push_str(&format!("// {}\n", spec.to_string()));
        if let Some(contents) = &module.source() {
          source.push_str(contents);
        }
      }
    }

    let out_path = out_dir.join(chunk.name.to_string());
    fs::write(&out_path, &source).unwrap();

    let out_len = out_path.to_string_lossy().len();
    if out_len > cols.0 {
      cols.0 = out_len;
    }

    let mut gzip_writer = ZlibEncoder::new(vec![], Compression::default());
    gzip_writer.write_all(source.as_bytes())?;
    let gzip_compressed = gzip_writer.finish()?;

    stats.push(BundleChunkStat {
      name: out_path.clone(),
      size: source.len(),
      gzip: gzip_compressed.len(),
      brotli: 0,
    });
  }

  // Sort to show biggest files first
  stats.sort_by(|a, b| {
    if a.gzip > b.gzip {
      Ordering::Greater
    } else if a.gzip < b.gzip {
      Ordering::Less
    } else {
      Ordering::Equal
    }
  });

  log::log!(
    log::Level::Info,
    "{}  {}  {}  {}",
    colors::green(&format!("{:<width$}", "Filename", width = cols.0 + 2)),
    colors::green("Size"),
    colors::green("Gzip"),
    colors::green("Brotli")
  );
  for stat in stats {
    log::log!(
      log::Level::Info,
      "  {}  {}  {}  {}",
      format!("{:<width$}", stat.name.to_string_lossy(), width = cols.0),
      colors::cyan(&format!("{:>width$}", stat.size, width = cols.1)),
      colors::cyan(&format!("{:>width$}", stat.gzip, width = cols.2)),
      colors::cyan(&format!("{:>width$}", stat.brotli, width = cols.3))
    );
  }
  log::log!(log::Level::Info, "");

  // eprintln!("chunk {:#?}", chunk_graph);

  Ok(())
}

fn assign_chunks(
  bundle_flags: &BundleFlags,
  chunk_graph: &mut ChunkGraph,
  graph: &HashMap<Url, Module>,
  npm_resolver: &Arc<dyn CliNpmResolver>,
  node_resolver: &Arc<NodeResolver>,
  cjs_tracker: &Arc<CjsTracker>,
  url: &Url,
  parent_chunk_id: Option<usize>,
  is_dynamic: bool,
) {
  let module = graph.get(url).unwrap();

  match module {
    Module::Js(js_module) => {
      let chunk_id = chunk_graph.assign_specifier_to_chunk(
        url,
        parent_chunk_id,
        ChunkKind::Js,
        is_dynamic,
      );

      for (_, dep) in &js_module.dependencies {
        match &dep.maybe_code {
          Resolution::None => todo!(),
          Resolution::Ok(resolution_resolved) => {
            assign_chunks(
              bundle_flags,
              chunk_graph,
              graph,
              npm_resolver,
              node_resolver,
              cjs_tracker,
              &resolution_resolved.specifier,
              Some(chunk_id),
              dep.is_dynamic,
            );
          }
          Resolution::Err(resolution_error) => todo!(),
        }
      }
    }
    Module::Json(json_module) => {
      chunk_graph.assign_specifier_to_chunk(
        url,
        parent_chunk_id,
        ChunkKind::Js,
        is_dynamic,
      );
    }
    Module::Wasm(wasm_module) => {
      chunk_graph.assign_specifier_to_chunk(
        url,
        parent_chunk_id,
        ChunkKind::Asset("wasm".to_string()),
        true,
      );
    }
    Module::Npm(_) => {
      unreachable!()
    }
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

#[derive(Debug, Eq, PartialEq)]
enum ChunkKind {
  Asset(String),
  Js,
}

#[derive(Debug)]
struct Chunk {
  id: usize,
  name: String,
  pub kind: ChunkKind,
  parent_ids: HashSet<usize>,
  children: Vec<usize>, // TODO: IndexSet?
  specifiers: IndexSet<Url>,
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

  fn assign_specifier_to_chunk(
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
