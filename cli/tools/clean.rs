// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::bail;
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
use crate::graph_util::CliJsrUrlProvider;
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
    return clean_entrypoint(
      flags,
      &clean_flags.entrypoints,
      clean_flags.dry_run,
    )
    .await;
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

#[derive(Clone, Debug)]
struct Node {
  id: usize,
  value: OsString,
  children: Vec<usize>,
}
#[derive(Default, Debug)]
struct Trie {
  nodes: Vec<Node>,
  roots: Vec<usize>,
}

impl Trie {
  fn new() -> Self {
    Self::default()
  }
  fn insert(&mut self, s: &Path) {
    let mut components =
      s.components().into_iter().map(|c| c.as_os_str()).peekable();
    let mut node = None;
    for &root_idx in &self.roots {
      let root = &self.nodes[root_idx];
      if components.next_if_eq(&root.value).is_some() {
        node = Some(root.clone());
      }
    }

    if node.is_none() {
      let id = self.nodes.len();
      self.nodes.push(Node {
        id,
        value: components.next().unwrap().to_os_string(),
        children: vec![],
      });
      self.roots.push(id);
      node = Some(self.nodes[id].clone());
    }

    let mut node = node.unwrap();

    'outer: while let Some(ch) = components.next() {
      let mut children = std::mem::take(&mut node.children);
      for &child in &children {
        let child = &self.nodes[child];
        if child.value == ch {
          node = child.clone();
          continue 'outer;
        }
      }

      let mut rest = components.rev();
      let mut child = None;
      while let Some(next) = rest.next() {
        let id = self.nodes.len();
        self.nodes.push(Node {
          id,
          value: next.to_os_string(),
          children: child.map(|id| vec![id]).unwrap_or_default(),
        });
        child = Some(id);
      }
      let id = self.nodes.len();
      self.nodes.push(Node {
        id,
        value: ch.to_os_string(),
        children: child.map(|id| vec![id]).unwrap_or_default(),
      });

      // eprintln!("hi: {node:?}");

      children.push(id);

      node.children = children;
      // eprintln!("hi: {node:?}");

      let node_id = node.id;
      self.nodes[node_id] = node;

      break;
    }
  }

  fn is_prefix(&self, s: &Path) -> (bool, bool) {
    let chars = s.components();

    let mut search = &self.roots;

    'outer: for c in chars {
      for &id in search {
        let node = &self.nodes[id];
        if node.value == c.as_os_str() {
          search = &node.children;
          continue 'outer;
        }
      }
      return (false, false);
    }

    (true, search.is_empty())
  }
}

async fn clean_entrypoint(
  flags: Arc<Flags>,
  entrypoints: &[String],
  dry_run: bool,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  let options = factory.cli_options()?;
  let main_graph_container = factory.main_module_graph_container().await?;
  let roots = main_graph_container.collect_specifiers(entrypoints)?;
  let http_cache = factory.global_http_cache()?;
  let deno_dir = factory.deno_dir()?;

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

  let npm_resolver = factory.npm_resolver().await?;

  let mut keep = HashSet::new();
  let mut npm_reqs = Vec::new();

  let mut keep_paths_trie = Trie::new();

  for (_, entry) in graph.walk(
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
          keep.insert(&js_module.specifier);
        }
        deno_graph::Module::Json(json_module) => {
          keep.insert(&json_module.specifier);
        }
        deno_graph::Module::Wasm(wasm_module) => {
          keep.insert(&wasm_module.specifier);
        }
        deno_graph::Module::Npm(npm_module) => {
          if let Some(managed) = npm_resolver.as_managed() {
            let id = managed
              .resolution()
              .resolve_pkg_id_from_deno_module(npm_module.nv_reference.nv())
              .unwrap();
            npm_reqs
              .extend(managed.resolution().resolve_pkg_reqs_from_pkg_id(&id));
          }
        }
        deno_graph::Module::Node(_) => {}
        deno_graph::Module::External(_) => {}
      },
      deno_graph::ModuleEntryRef::Err(_) => {}
      deno_graph::ModuleEntryRef::Redirect(_) => {}
    }
  }

  for url in keep {
    if url.scheme() == "http" || url.scheme() == "https" {
      if let Ok(path) = http_cache.local_path_for_url(url) {
        keep_paths_trie.insert(&path);
      }
    }
    if let Some(path) = deno_dir
      .gen_cache
      .get_cache_filename_with_extension(url, "js")
    {
      let path = deno_dir.gen_cache.location.join(path);
      keep_paths_trie.insert(&path);
    }
  }

  let npm_cache = factory.npm_cache()?;
  let snap = npm_resolver.as_managed().unwrap().resolution().snapshot();
  for package in snap.all_system_packages(&options.npm_system_info()) {
    keep_paths_trie.insert(
      &npm_cache
        .package_name_folder(&package.id.nv.name)
        .join("registry.json"),
    );
  }
  let snap = snap.subset(&npm_reqs);
  let node_modules_path = npm_resolver.root_node_modules_path();
  let mut node_modules_keep = HashSet::new();
  for package in snap.all_system_packages(&options.npm_system_info()) {
    if node_modules_path.is_some() {
      node_modules_keep.insert(package.get_package_cache_folder_id());
    }
    keep_paths_trie.insert(&npm_cache.package_folder_for_id(
      &deno_npm::NpmPackageCacheFolderId {
        nv: package.id.nv.clone(),
        copy_index: package.copy_index,
      },
    ));
  }

  let jsr_url = crate::args::jsr_url();

  for package in graph.packages.mappings().values() {
    let Ok(base_url) =
      (if let Some((scope, name)) = package.name.split_once('/') {
        jsr_url
          .join(&format!("{}/", scope))
          .and_then(|u| u.join(&format!("{}/", name)))
      } else {
        jsr_url.join(&format!("{}/", &package.name))
      })
    else {
      continue;
    };
    let keep =
      http_cache.local_path_for_url(&base_url.join("meta.json").unwrap())?;
    keep_paths_trie.insert(&keep);
    let keep = http_cache.local_path_for_url(
      &base_url
        .join(&format!("{}_meta.json", package.version))
        .unwrap(),
    )?;
    keep_paths_trie.insert(&keep);
  }
  let mut walker = walkdir::WalkDir::new(&deno_dir.root)
    .contents_first(false)
    .min_depth(2)
    .into_iter();
  while let Some(entry) = walker.next() {
    let entry = entry?;
    let (is_prefix, is_match) = keep_paths_trie.is_prefix(entry.path());
    if is_prefix {
      if entry.file_type().is_dir() && is_match {
        walker.skip_current_dir();
        continue;
      }
      continue;
    }
    if !entry.path().starts_with(&deno_dir.root) {
      panic!("VERY BAD");
    }
    if entry.file_type().is_dir() {
      if dry_run {
        eprintln!("would remove dir: {}", entry.path().display());
      } else {
        std::fs::remove_dir_all(entry.path())?;
      }
      walker.skip_current_dir();
    } else {
      if dry_run {
        eprintln!("would remove file: {}", entry.path().display());
      } else {
        std::fs::remove_file(entry.path())?;
      }
    }
  }

  if let Some(dir) = node_modules_path {
    clean_node_modules(&node_modules_keep, dir, dry_run)?;
  }

  Ok(())
}

fn clean_node_modules(
  keep_pkgs: &HashSet<deno_npm::NpmPackageCacheFolderId>,
  dir: &Path,
  dry_run: bool,
) -> Result<(), AnyError> {
  if !dir.ends_with("node_modules") || !dir.is_dir() {
    bail!("not a node_modules directory");
  }
  if !dir.join(".deno").exists() {
    return Ok(());
  }

  let base = dir.join(".deno");
  let entries = std::fs::read_dir(base)?;

  let keep_names = keep_pkgs
    .iter()
    .map(|id| deno_resolver::npm::get_package_folder_id_folder_name(id))
    .collect::<HashSet<_>>();

  eprintln!("keep_names: {keep_names:?}");
  for entry in entries {
    let entry = entry?;
    if !entry.file_type()?.is_dir() {
      continue;
    }
    if keep_names.contains(entry.file_name().to_string_lossy().as_ref()) {
      continue;
    } else {
      if dry_run {
        eprintln!("removing from node modules: {}", entry.path().display());
      } else {
        std::fs::remove_dir_all(entry.path())?;
      }
    }
  }

  let top_level = std::fs::read_dir(dir)?;
  for entry in top_level {
    let entry = entry?;
    let ty = entry.file_type()?;

    if ty.is_symlink() {
      let target = std::fs::read_link(entry.path());
      let remove = if let Ok(target) = target {
        let path = dir.join(target);
        !path.exists()
      } else {
        true
      };
      if remove {
        if dry_run {
          eprintln!(
            "removing top level symlink from node modules: {}",
            entry.path().display()
          );
        } else {
          std::fs::remove_file(entry.path())?;
        }
      }
    }
  }

  Ok(())
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
