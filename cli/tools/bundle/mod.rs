use std::{
  cmp::Ordering,
  fs,
  io::Write,
  path::{Path, PathBuf},
  sync::Arc,
};

use bundle_graph::BundleModule;
use bundle_resolver::build_resolved_graph;
use chunk_graph::{assign_chunks, ChunkGraph};
use deno_core::error::AnyError;
use deno_runtime::colors;
use flate2::{write::ZlibEncoder, Compression};

use crate::{
  args::{BundleFlags, Flags},
  factory::CliFactory,
  util::{fs::collect_specifiers, path::matches_pattern_or_exact_path},
};

mod bundle_graph;
mod bundle_resolver;
mod chunk_graph;
mod transform;

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

  let bundle_graph = build_resolved_graph(
    module_graph_creator,
    npm_resolver,
    node_resolver,
    files.clone(),
  )
  .await?;

  let mut chunk_graph = ChunkGraph::new();
  for file in files {
    assign_chunks(
      &bundle_flags,
      &mut chunk_graph,
      &bundle_graph,
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
      if let Some(module) = bundle_graph.get(&spec) {
        // FIXME: don't print module urls by default
        source.push_str(&format!("// {}\n", spec.to_string()));
        match module {
          BundleModule::Js(bundle_js_module) => {
            source.push_str(&bundle_js_module.source);
          }
          BundleModule::Json(json_module) => todo!(),
          BundleModule::Wasm(wasm_module) => todo!(),
          BundleModule::Node(_) => todo!(),
          BundleModule::External(external_module) => todo!(),
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
