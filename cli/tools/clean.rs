// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_error::JsErrorBox;
use deno_graph::packages::PackageSpecifiers;
use deno_graph::source::LoadError;
use deno_graph::source::Loader;
use node_resolver::UrlOrPathRef;

use crate::args::CleanFlags;
use crate::args::Flags;
use crate::colors;
use crate::display;
use crate::factory::CliFactory;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::graph_util::CreateGraphOptions;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::progress_bar::ProgressMessagePrompt;
use crate::util::progress_bar::UpdateGuard;

struct CleanState {
  files_removed: u64,
  dirs_removed: u64,
  bytes_removed: u64,
  progress_guard: UpdateGuard,
}

impl CleanState {
  fn update_progress(&self) {
    self
      .progress_guard
      .set_position(self.files_removed + self.dirs_removed);
  }
}

pub async fn clean(
  flags: Arc<Flags>,
  clean_flags: CleanFlags,
) -> Result<(), AnyError> {
  if !clean_flags.entrypoints.is_empty() {
    return clean_entrypoint(flags, &clean_flags.entrypoints).await;
  }

  let factory = CliFactory::from_flags(flags);
  let deno_dir = factory.deno_dir()?;
  if deno_dir.root.exists() {
    let no_of_files = walkdir::WalkDir::new(&deno_dir.root).into_iter().count();
    let progress_bar = ProgressBar::new(ProgressBarStyle::ProgressBars);
    let progress_guard =
      progress_bar.update_with_prompt(ProgressMessagePrompt::Cleaning, "");

    let mut state = CleanState {
      files_removed: 0,
      dirs_removed: 0,
      bytes_removed: 0,
      progress_guard,
    };
    state
      .progress_guard
      .set_total_size(no_of_files.try_into().unwrap());

    rm_rf(&mut state, &deno_dir.root)?;

    // Drop the guard so that progress bar disappears.
    drop(state.progress_guard);

    log::info!(
      "{} {} {}",
      colors::green("Removed"),
      deno_dir.root.display(),
      colors::gray(&format!(
        "({} files, {})",
        state.files_removed + state.dirs_removed,
        display::human_size(state.bytes_removed as f64)
      ))
    );
  }

  Ok(())
}

#[derive(Clone)]
struct Node {
  value: char,
  children: Vec<usize>,
}
struct Trie {
  nodes: Vec<Node>,
  roots: Vec<usize>,
}

impl Trie {
  fn insert(&mut self, s: &str) {
    let mut chars = s.chars().peekable();
    let mut node = Node {
      value: '\0',
      children: vec![],
    };
    for &root_idx in &self.roots {
      let root = &self.nodes[root_idx];
      if chars.next_if_eq(&root.value).is_some() {
        node = root.clone();
      }
    }

    'outer: while let Some(ch) = chars.next() {
      let children = std::mem::take(&mut node.children);
      for child in children {
        let child = &self.nodes[child];
        if child.value == ch {
          node = child.clone();
          continue 'outer;
        }
      }

      while let Some(next) = chars.next() {
          chars.rev()
      }
      children.push(Node {
        value: ch,
        children: vec![],
      })
    }
  }
}

async fn clean_entrypoint(
  flags: Arc<Flags>,
  entrypoints: &[String],
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  let options = factory.cli_options()?;
  let main_graph_container = factory.main_module_graph_container().await?;
  let roots = main_graph_container.collect_specifiers(entrypoints)?;
  let http_cache = factory.global_http_cache()?;
  let mut permit = main_graph_container.acquire_update_permit().await;
  let graph = permit.graph_mut();
  graph.packages = PackageSpecifiers::default();
  let graph_builder = factory.module_graph_builder().await?;
  graph_builder
    .build_graph_with_npm_resolution(
      graph,
      CreateGraphOptions {
        // loader: Some(&mut NoLoader),
        loader: None,
        graph_kind: graph.graph_kind(),
        is_dynamic: false,
        roots: roots.clone(),
        npm_caching: crate::graph_util::NpmCachingStrategy::Manual,
      },
    )
    .await?;

  let node_resolver = factory.node_resolver().await?;
  let npm_resolver = factory.npm_resolver().await?;

  let mut keep = HashSet::new();
  let mut keep_paths = HashSet::new();
  let mut keep_dirs = HashSet::new();

  for (specifier, entry) in graph.walk(
    roots.iter(),
    deno_graph::WalkOptions {
      check_js: deno_graph::CheckJsOption::False,
      follow_dynamic: true,
      kind: graph.graph_kind(),
      prefer_fast_check_graph: false,
    },
  ) {
    match entry {
      deno_graph::ModuleEntryRef::Module(module) => match module {
        deno_graph::Module::Js(js_module) => {
          keep_dirs.insert(&js_module.specifier);
          for (_, m) in js_module.dependencies.iter() {
            if let Some(code) = m.get_code() {}
          }
        }
        deno_graph::Module::Json(json_module) => {
          keep.insert(&json_module.specifier);
        }
        deno_graph::Module::Wasm(wasm_module) => {
          keep.insert(&wasm_module.specifier);
        }
        deno_graph::Module::Npm(npm_module) => {
          if let Some(managed) = npm_resolver.as_managed() {
            if let Some(package_folder) = managed
              .resolve_pkg_folder_from_deno_module(npm_module.nv_reference.nv())
              .ok()
            {
              keep_paths.insert(package_folder);
            }
          }

          eprintln!(
            "npm specifier: {} {}",
            npm_module.nv_reference, npm_module.specifier
          );
        }
        deno_graph::Module::Node(built_in_node_module) => {}
        deno_graph::Module::External(external_module) => {}
      },
      deno_graph::ModuleEntryRef::Err(module_error) => {
        eprintln!("error: {module_error}");
      }
      deno_graph::ModuleEntryRef::Redirect(url) => {}
    }
  }

  for url in keep {
    if url.scheme() == "http" || url.scheme() == "https" {
      if let Ok(path) = http_cache.local_path_for_url(url) {
        keep_paths.insert(path);
      } else {
        eprintln!("very bad not good: {url}");
      }
    } else {
      eprintln!("bad bad not good: {url}");
    }
  }
  dbg!(keep_paths);
  let deno_dir = factory.deno_dir()?;
  eprintln!("deno_dir: {}", deno_dir.root.display());
  let node_modules_path = npm_resolver.root_node_modules_path();
  for entry in walkdir::WalkDir::new(&deno_dir.root).contents_first(false) {
    let entry = entry?;
    if entry.file_type().is_dir() {}
  }

  Ok(())
}

struct NoLoader;

impl Loader for NoLoader {
  fn load(
    &self,
    _specifier: &deno_ast::ModuleSpecifier,
    _options: deno_graph::source::LoadOptions,
  ) -> deno_graph::source::LoadFuture {
    std::future::ready(Err(LoadError::Other(Arc::new(
      JsErrorBox::not_supported(),
    ))))
    .boxed_local()
  }
}

fn rm_rf(state: &mut CleanState, path: &Path) -> Result<(), AnyError> {
  for entry in walkdir::WalkDir::new(path).contents_first(true) {
    let entry = entry?;

    if entry.file_type().is_dir() {
      state.dirs_removed += 1;
      state.update_progress();
      std::fs::remove_dir_all(entry.path())?;
    } else {
      remove_file(state, entry.path(), entry.metadata().ok())?;
    }
  }

  Ok(())
}

fn remove_file(
  state: &mut CleanState,
  path: &Path,
  meta: Option<std::fs::Metadata>,
) -> Result<(), AnyError> {
  if let Some(meta) = meta {
    state.bytes_removed += meta.len();
  }
  state.files_removed += 1;
  state.update_progress();
  std::fs::remove_file(path)
    .with_context(|| format!("Failed to remove file: {}", path.display()))?;
  Ok(())
}
