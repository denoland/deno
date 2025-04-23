// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_cache_dir::GlobalOrLocalHttpCache;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::packages::PackageSpecifiers;
use deno_graph::ModuleGraph;
use walkdir::WalkDir;

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

#[derive(Default)]
struct CleanState {
  files_removed: u64,
  dirs_removed: u64,
  bytes_removed: u64,
  progress_guard: Option<UpdateGuard>,
}

impl CleanState {
  fn update_progress(&self) {
    if let Some(pg) = &self.progress_guard {
      pg.set_position(self.files_removed + self.dirs_removed);
    }
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
    progress_guard.set_total_size(no_of_files.try_into().unwrap());
    let mut state = CleanState {
      files_removed: 0,
      dirs_removed: 0,
      bytes_removed: 0,
      progress_guard: Some(progress_guard),
    };

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

#[derive(Clone, Debug, Default)]
struct PathNode {
  exact: bool,
  children: BTreeMap<OsString, usize>,
}
#[derive(Debug)]
struct PathTrie {
  root: usize,
  nodes: Vec<PathNode>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Found {
  Match,
  Prefix,
}

impl PathTrie {
  fn new() -> Self {
    Self {
      root: 0,
      nodes: vec![PathNode {
        exact: false,
        children: Default::default(),
      }],
    }
  }
  fn insert(&mut self, s: &Path) {
    let mut components = s.components().into_iter().map(|c| c.as_os_str());
    let mut node = self.root;

    while let Some(component) = components.next() {
      if let Some(nd) = self.nodes[node].children.get(component).copied() {
        node = nd;
      } else {
        let id = self.nodes.len();
        self.nodes.push(PathNode::default());
        self.nodes[node]
          .children
          .insert(component.to_os_string(), id);
        node = id;
      }
    }

    self.nodes[node].exact = true;
  }

  fn find(&self, s: &Path) -> Option<Found> {
    let mut components = s.components().into_iter().map(|c| c.as_os_str());
    let mut node = self.root;

    while let Some(component) = components.next() {
      if let Some(nd) = self.nodes[node].children.get(component).copied() {
        node = nd;
      } else {
        return None;
      }
    }

    Some(if self.nodes[node].exact {
      Found::Match
    } else {
      Found::Prefix
    })
  }
}

async fn clean_entrypoint(
  flags: Arc<Flags>,
  entrypoints: &[String],
  dry_run: bool,
) -> Result<(), AnyError> {
  let mut state = CleanState::default();

  let factory = CliFactory::from_flags(flags.clone());
  let options = factory.cli_options()?;
  let main_graph_container = factory.main_module_graph_container().await?;
  let roots = main_graph_container.collect_specifiers(entrypoints)?;
  let http_cache = factory.global_http_cache()?;
  let local_or_global_http_cache = factory.http_cache()?.clone();
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

  let mut keep_paths_trie = PathTrie::new();

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

  for url in &keep {
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
  // TODO(nathanwhit): remove once we don't need packuments for creating the snapshot from lockfile
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
  add_jsr_meta_paths(&graph, &mut keep_paths_trie, jsr_url, &|url| {
    http_cache.local_path_for_url(url).map_err(Into::into)
  })?;
  walk_removing(
    &mut state,
    walkdir::WalkDir::new(&deno_dir.root)
      .contents_first(false)
      .min_depth(2),
    &keep_paths_trie,
    &deno_dir.root,
    dry_run,
  )?;
  let mut node_modules_cleaned = CleanState::default();

  if let Some(dir) = node_modules_path {
    clean_node_modules(
      &mut node_modules_cleaned,
      &node_modules_keep,
      dir,
      dry_run,
    )?;
  }

  let mut vendor_cleaned = CleanState::default();
  if let Some(vendor_dir) = options.vendor_dir_path() {
    if let GlobalOrLocalHttpCache::Local(cache) = local_or_global_http_cache {
      let mut trie = PathTrie::new();
      let cache = cache.clone();
      add_jsr_meta_paths(&graph, &mut trie, jsr_url, &|_url| {
        if let Ok(Some(path)) = cache.local_path_for_url(_url) {
          Ok(path)
        } else {
          panic!("should not happen")
        }
      })?;
      for url in keep {
        if url.scheme() == "http" || url.scheme() == "https" {
          if let Ok(Some(path)) = cache.local_path_for_url(url) {
            trie.insert(&path);
          } else {
            panic!("should not happen")
          }
        }
      }

      walk_removing(
        &mut vendor_cleaned,
        WalkDir::new(vendor_dir).contents_first(false),
        &trie,
        &vendor_dir,
        dry_run,
      )?;
    }
  }

  if !dry_run {
    log_stats(&state, &deno_dir.root);

    if let Some(dir) = node_modules_path {
      log_stats(&node_modules_cleaned, dir);
    }
    if let Some(dir) = options.vendor_dir_path() {
      log_stats(&vendor_cleaned, dir);
    }
  }

  Ok(())
}

fn log_stats(state: &CleanState, dir: &Path) {
  if state.bytes_removed == 0
    && state.dirs_removed == 0
    && state.files_removed == 0
  {
    return;
  }
  log::info!(
    "{} {}",
    colors::green("Removed"),
    colors::gray(&format!(
      "{} files, {} from {}",
      state.files_removed + state.dirs_removed,
      display::human_size(state.bytes_removed as f64),
      dir.display()
    ))
  );
}

fn add_jsr_meta_paths(
  graph: &ModuleGraph,
  path_trie: &mut PathTrie,
  jsr_url: &Url,
  url_to_path: &dyn Fn(&Url) -> Result<PathBuf, AnyError>,
) -> Result<(), AnyError> {
  for package in graph.packages.mappings().values() {
    let Ok(base_url) = jsr_url.join(&format!("{}/", &package.name)) else {
      continue;
    };
    let keep = url_to_path(&base_url.join("meta.json").unwrap())?;
    path_trie.insert(&keep);
    let keep = url_to_path(
      &base_url
        .join(&format!("{}_meta.json", package.version))
        .unwrap(),
    )?;
    path_trie.insert(&keep);
  }
  Ok(())
}

fn walk_removing(
  state: &mut CleanState,
  walker: WalkDir,
  trie: &PathTrie,
  base: &Path,
  dry_run: bool,
) -> Result<(), AnyError> {
  let mut walker = walker.into_iter();
  while let Some(entry) = walker.next() {
    let entry = entry?;
    if let Some(found) = trie.find(entry.path()) {
      if entry.file_type().is_dir() && matches!(found, Found::Match) {
        walker.skip_current_dir();
        continue;
      }
      continue;
    }
    if !entry.path().starts_with(base) {
      panic!("VERY BAD");
    }
    if entry.file_type().is_dir() {
      if dry_run {
        eprintln!("would remove dir: {}", entry.path().display());
      } else {
        rm_rf(state, entry.path())?;
      }
      walker.skip_current_dir();
    } else {
      if dry_run {
        eprintln!("would remove file: {}", entry.path().display());
      } else {
        remove_file(state, entry.path(), Some(entry.metadata()?))?;
      }
    }
  }

  Ok(())
}

fn clean_node_modules(
  state: &mut CleanState,
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

  let keep_names = keep_pkgs
    .iter()
    .map(|id| deno_resolver::npm::get_package_folder_id_folder_name(id))
    .collect::<HashSet<_>>();

  let base = dir.join(".deno");
  let entries = std::fs::read_dir(&base)?;
  for entry in entries {
    let entry = entry?;
    if !entry.file_type()?.is_dir() {
      continue;
    }
    let file_name = entry.file_name();
    let file_name = file_name.to_string_lossy();
    if keep_names.contains(file_name.as_ref()) || file_name == "node_modules" {
      continue;
    } else {
      if dry_run {
        eprintln!("removing from node modules: {}", entry.path().display());
      } else {
        rm_rf(state, &entry.path())?;
      }
    }
  }

  clean_node_modules_symlinks(state, &keep_names, dir, dry_run)?;

  clean_node_modules_symlinks(
    state,
    &keep_names,
    &base.join("node_modules"),
    dry_run,
  )?;
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
          remove_file(state, &entry.path(), None)?;
        }
      }
    }
  }

  Ok(())
}

fn clean_node_modules_symlinks(
  state: &mut CleanState,
  keep_names: &HashSet<String>,
  dir: &Path,
  dry_run: bool,
) -> Result<(), AnyError> {
  for entry in std::fs::read_dir(dir)? {
    let entry = entry?;
    let ty = entry.file_type()?;
    if ty.is_symlink() {
      let target = std::fs::read_link(entry.path())?;
      if !keep_names.contains(
        &*target
          .parent()
          .unwrap()
          .parent()
          .unwrap()
          .file_name()
          .unwrap()
          .to_string_lossy(),
      ) {
        if dry_run {
          eprintln!(
            "removing top level symlink from node modules: {}",
            entry.path().display()
          );
        } else {
          remove_file(state, &entry.path(), None)?;
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

#[cfg(test)]
mod tests {
  use super::Found::*;

  #[cfg(unix)]
  #[test]
  fn path_trie() {
    use std::path::Path;

    let mut trie = super::PathTrie::new();

    let paths = {
      #[cfg(unix)]
      {
        [
          "/foo/bar/deno",
          "/foo/bar/deno/1",
          "/foo/bar/deno/2",
          "/foo/baz",
        ]
      }
      #[cfg(windows)]
      {
        [
          r"C:\foo\bar\deno",
          r"C:\foo\bar\deno\1",
          r"C:\foo\bar\deno\2",
          r"C:\foo\baz",
          r"D:\thing",
        ]
      }
    };

    let cases = {
      #[cfg(unix)]
      {
        [
          ("/", Some(Prefix)),
          ("/foo", Some(Prefix)),
          ("/foo/", Some(Prefix)),
          ("/foo/bar", Some(Prefix)),
          ("/foo/bar/deno", Some(Match)),
          ("/foo/bar/deno/1", Some(Match)),
          ("/foo/bar/deno/2", Some(Match)),
          ("/foo/baz", Some(Match)),
          ("/fo", None),
          ("/foo/baz/deno", None),
        ]
      }
      #[cfg(windows)]
      {
        [
          (r"C:\", Some(Prefix)),
          (r"C:\foo", Some(Prefix)),
          (r"C:\foo\", Some(Prefix)),
          (r"C:\foo\", Some(Prefix)),
          (r"C:\foo\bar", Some(Match)),
          (r"C:\foo\bar\deno\1", Some(Match)),
          (r"C:\foo\bar\deno\2", Some(Match)),
          (r"C:\foo\baz", Some(Match)),
          (r"C:\fo", None),
          (r"C:\foo\baz\deno", None),
          (r"D:\", Some(Prefix)),
          (r"E:\", None),
        ]
      }
    };

    for pth in paths {
      let path = Path::new(pth);
      trie.insert(path);
    }

    for (input, expect) in cases {
      let path = Path::new(input);
      assert_eq!(trie.find(path), expect, "on input: {input}");
    }
  }
}
