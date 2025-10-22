// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_cache_dir::GlobalOrLocalHttpCache;
use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::ModuleGraph;
use deno_graph::packages::PackageSpecifiers;
use deno_npm_installer::graph::NpmCachingStrategy;
use sys_traits::FsCanonicalize;
use sys_traits::FsCreateDirAll;
use walkdir::WalkDir;

use crate::args::CleanFlags;
use crate::args::Flags;
use crate::colors;
use crate::display;
use crate::factory::CliFactory;
use crate::graph_container::CollectSpecifiersOptions;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::graph_util::BuildGraphRequest;
use crate::graph_util::BuildGraphWithNpmOptions;
use crate::sys::CliSys;
use crate::util::fs::FsCleaner;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;
use crate::util::progress_bar::ProgressMessagePrompt;

pub async fn clean(
  flags: Arc<Flags>,
  clean_flags: CleanFlags,
) -> Result<(), AnyError> {
  if !clean_flags.except_paths.is_empty() {
    return clean_except(flags, &clean_flags.except_paths, clean_flags.dry_run)
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
    let mut cleaner = FsCleaner::new(Some(progress_guard));

    cleaner.rm_rf(&deno_dir.root)?;

    // Drop the guard so that progress bar disappears.
    drop(cleaner.progress_guard);

    log::info!(
      "{} {} {}",
      colors::green("Removed"),
      deno_dir.root.display(),
      colors::gray(&format!(
        "({} files, {})",
        cleaner.files_removed + cleaner.dirs_removed,
        display::human_size(cleaner.bytes_removed as f64)
      ))
    );
  }

  Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Found {
  Match,
  Prefix,
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
  rewrites: Vec<(PathBuf, PathBuf)>,
}

impl PathTrie {
  fn new() -> Self {
    Self {
      root: 0,
      nodes: vec![PathNode {
        exact: false,
        children: Default::default(),
      }],
      rewrites: vec![],
    }
  }

  fn add_rewrite(&mut self, from: PathBuf, to: PathBuf) {
    self.rewrites.push((from, to));
  }

  fn rewrite<'a>(&self, s: Cow<'a, Path>) -> Cow<'a, Path> {
    let normalized = deno_path_util::normalize_path(s);
    for (from, to) in &self.rewrites {
      if normalized.starts_with(from) {
        return Cow::Owned(to.join(normalized.strip_prefix(from).unwrap()));
      }
    }
    normalized
  }

  fn insert(&mut self, s: PathBuf) {
    let normalized = self.rewrite(Cow::Owned(s));
    let components = normalized.components().map(|c| c.as_os_str());
    let mut node = self.root;

    for component in components {
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
    let normalized = self.rewrite(Cow::Borrowed(s));
    let components = normalized.components().map(|c| c.as_os_str());
    let mut node = self.root;

    for component in components {
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

fn try_get_canonicalized_root_dir<Sys: FsCanonicalize + FsCreateDirAll>(
  sys: &Sys,
  root_dir: &Path,
) -> Result<PathBuf, std::io::Error> {
  match sys.fs_canonicalize(root_dir) {
    Ok(path) => Ok(path),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
      sys.fs_create_dir_all(root_dir)?;
      sys.fs_canonicalize(root_dir)
    }
    Err(err) => Err(err),
  }
}

async fn clean_except(
  flags: Arc<Flags>,
  entrypoints: &[String],
  dry_run: bool,
) -> Result<(), AnyError> {
  let mut state = FsCleaner::default();

  let factory = CliFactory::from_flags(flags.clone());
  let sys = factory.sys();
  let options = factory.cli_options()?;
  let main_graph_container = factory.main_module_graph_container().await?;
  let roots = main_graph_container.collect_specifiers(
    entrypoints,
    CollectSpecifiersOptions {
      include_ignored_specified: true,
    },
  )?;
  let http_cache = factory.global_http_cache()?;
  let local_or_global_http_cache = factory.http_cache()?.clone();
  let deno_dir = factory.deno_dir()?.clone();
  let deno_dir_root_canonical =
    try_get_canonicalized_root_dir(&sys, &deno_dir.root)
      .unwrap_or(deno_dir.root.clone());

  let mut permit = main_graph_container.acquire_update_permit().await;
  let graph = permit.graph_mut();
  graph.packages = PackageSpecifiers::default();
  let graph_builder = factory.module_graph_builder().await?;
  graph_builder
    .build_graph_with_npm_resolution(
      graph,
      BuildGraphWithNpmOptions {
        request: BuildGraphRequest::Roots(roots.clone()),
        loader: None,
        is_dynamic: false,
        npm_caching: NpmCachingStrategy::Manual,
      },
    )
    .await?;

  let npm_resolver = factory.npm_resolver().await?;

  let mut keep = HashSet::new();
  let mut npm_reqs = Vec::new();

  let mut keep_paths_trie = PathTrie::new();
  if deno_dir_root_canonical != deno_dir.root {
    keep_paths_trie
      .add_rewrite(deno_dir.root.clone(), deno_dir_root_canonical.clone());
  }
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
            // TODO(dsherret): ok to use for now, but we should use the req in the future
            #[allow(deprecated)]
            let nv = npm_module.nv_reference.nv();
            let id = managed
              .resolution()
              .resolve_pkg_id_from_deno_module(nv)
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
    if (url.scheme() == "http" || url.scheme() == "https")
      && let Ok(path) = http_cache.local_path_for_url(url)
    {
      keep_paths_trie.insert(path);
    }
    if let Some(path) = deno_dir
      .gen_cache
      .get_cache_filename_with_extension(url, "js")
    {
      let path = deno_dir.gen_cache.location.join(path);
      keep_paths_trie.insert(path);
    }
  }

  let npm_cache = factory.npm_cache()?;
  let snap = npm_resolver.as_managed().unwrap().resolution().snapshot();
  // TODO(nathanwhit): remove once we don't need packuments for creating the snapshot from lockfile
  for package in snap.all_system_packages(&options.npm_system_info()) {
    keep_paths_trie.insert(
      npm_cache
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
    keep_paths_trie.insert(npm_cache.package_folder_for_id(
      &deno_npm::NpmPackageCacheFolderId {
        nv: package.id.nv.clone(),
        copy_index: package.copy_index,
      },
    ));
  }

  if dry_run {
    #[allow(clippy::print_stderr)]
    {
      eprintln!("would remove:");
    }
  }

  let jsr_url = crate::args::jsr_url();
  add_jsr_meta_paths(graph, &mut keep_paths_trie, jsr_url, &|url| {
    http_cache
      .local_path_for_url(url)
      .map_err(Into::into)
      .map(Some)
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
  let mut node_modules_cleaned = FsCleaner::default();

  if let Some(dir) = node_modules_path {
    clean_node_modules(
      &mut node_modules_cleaned,
      &node_modules_keep,
      dir,
      dry_run,
    )?;
  }

  let mut vendor_cleaned = FsCleaner::default();
  if let Some(vendor_dir) = options.vendor_dir_path()
    && let GlobalOrLocalHttpCache::Local(cache) = local_or_global_http_cache
  {
    let mut trie = PathTrie::new();
    if deno_dir_root_canonical != deno_dir.root {
      trie.add_rewrite(deno_dir.root.clone(), deno_dir_root_canonical);
    }
    let cache = cache.clone();
    add_jsr_meta_paths(graph, &mut trie, jsr_url, &|url| match cache
      .local_path_for_url(url)
    {
      Ok(path) => Ok(path),
      Err(err) => {
        log::warn!(
          "failed to get local path for jsr meta url {}: {}",
          url,
          err
        );
        Ok(None)
      }
    })?;
    for url in keep {
      if url.scheme() == "http" || url.scheme() == "https" {
        match cache.local_path_for_url(url) {
          Ok(Some(path)) => {
            trie.insert(path);
          }
          Ok(None) => {}
          Err(err) => {
            log::warn!("failed to get local path for url {}: {}", url, err);
          }
        }
      }
    }

    walk_removing(
      &mut vendor_cleaned,
      WalkDir::new(vendor_dir).contents_first(false),
      &trie,
      vendor_dir,
      dry_run,
    )?;
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

fn log_stats(cleaner: &FsCleaner, dir: &Path) {
  if cleaner.bytes_removed == 0
    && cleaner.dirs_removed == 0
    && cleaner.files_removed == 0
  {
    return;
  }
  log::info!(
    "{} {}",
    colors::green("Removed"),
    colors::gray(&format!(
      "{} files, {} from {}",
      cleaner.files_removed + cleaner.dirs_removed,
      display::human_size(cleaner.bytes_removed as f64),
      dir.display()
    ))
  );
}

fn add_jsr_meta_paths(
  graph: &ModuleGraph,
  path_trie: &mut PathTrie,
  jsr_url: &Url,
  url_to_path: &dyn Fn(&Url) -> Result<Option<PathBuf>, AnyError>,
) -> Result<(), AnyError> {
  for package in graph.packages.mappings().values() {
    let Ok(base_url) = jsr_url.join(&format!("{}/", &package.name)) else {
      continue;
    };
    let keep = url_to_path(&base_url.join("meta.json").unwrap())?;
    if let Some(keep) = keep {
      path_trie.insert(keep);
    }
    let keep = url_to_path(
      &base_url
        .join(&format!("{}_meta.json", package.version))
        .unwrap(),
    )?;
    if let Some(keep) = keep {
      path_trie.insert(keep);
    }
  }
  Ok(())
}

// TODO(nathanwhit): use strategy pattern instead of branching on dry_run
fn walk_removing(
  cleaner: &mut FsCleaner,
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
      panic!(
        "would have removed a file outside of the base directory: base: {}, path: {}",
        base.display(),
        entry.path().display()
      );
    }
    if entry.file_type().is_dir() {
      if dry_run {
        #[allow(clippy::print_stderr)]
        {
          eprintln!(" {}", entry.path().display());
        }
      } else {
        cleaner.rm_rf(entry.path())?;
      }
      walker.skip_current_dir();
    } else if dry_run {
      #[allow(clippy::print_stderr)]
      {
        eprintln!(" {}", entry.path().display());
      }
    } else {
      cleaner.remove_file(entry.path(), Some(entry.metadata()?))?;
    }
  }

  Ok(())
}

fn clean_node_modules(
  cleaner: &mut FsCleaner,
  keep_pkgs: &HashSet<deno_npm::NpmPackageCacheFolderId>,
  dir: &Path,
  dry_run: bool,
) -> Result<(), AnyError> {
  if !dir.ends_with("node_modules") || !dir.is_dir() {
    bail!("expected a node_modules directory, got: {}", dir.display());
  }
  let base = dir.join(".deno");
  if !base.exists() {
    return Ok(());
  }

  let keep_names = keep_pkgs
    .iter()
    .map(deno_resolver::npm::get_package_folder_id_folder_name)
    .collect::<HashSet<_>>();

  // remove the actual packages from node_modules/.deno
  let entries = match std::fs::read_dir(&base) {
    Ok(entries) => entries,
    Err(err)
      if matches!(
        err.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
      ) =>
    {
      return Ok(());
    }
    Err(err) => {
      return Err(err).with_context(|| {
        format!(
          "failed to clean node_modules directory at {}",
          dir.display()
        )
      });
    }
  };

  // TODO(nathanwhit): this probably shouldn't reach directly into this code
  let mut setup_cache = deno_npm_installer::LocalSetupCache::load(
    CliSys::default(),
    base.join(".setup-cache.bin"),
  );

  for entry in entries {
    let entry = entry?;
    if !entry.file_type()?.is_dir() {
      continue;
    }
    let file_name = entry.file_name();
    let file_name = file_name.to_string_lossy();
    if keep_names.contains(file_name.as_ref()) || file_name == "node_modules" {
      continue;
    } else if dry_run {
      #[allow(clippy::print_stderr)]
      {
        eprintln!(" {}", entry.path().display());
      }
    } else {
      cleaner.rm_rf(&entry.path())?;
    }
  }

  // remove top level symlinks from node_modules/<package> to node_modules/.deno/<package>
  // where the target doesn't exist (because it was removed above)
  clean_node_modules_symlinks(
    cleaner,
    &keep_names,
    dir,
    dry_run,
    &mut |name| {
      setup_cache.remove_root_symlink(name);
    },
  )?;

  // remove symlinks from node_modules/.deno/node_modules/<package> to node_modules/.deno/<package>
  // where the target doesn't exist (because it was removed above)
  clean_node_modules_symlinks(
    cleaner,
    &keep_names,
    &base.join("node_modules"),
    dry_run,
    &mut |name| {
      setup_cache.remove_deno_symlink(name);
    },
  )?;
  if !dry_run {
    setup_cache.save();
  }

  Ok(())
}

// node_modules/.deno/chalk@5.0.1/node_modules/chalk -> chalk@5.0.1
fn node_modules_package_actual_dir_to_name(
  path: &Path,
) -> Option<Cow<'_, str>> {
  path
    .parent()?
    .parent()?
    .file_name()
    .map(|name| name.to_string_lossy())
}

fn clean_node_modules_symlinks(
  cleaner: &mut FsCleaner,
  keep_names: &HashSet<String>,
  dir: &Path,
  dry_run: bool,
  on_remove: &mut dyn FnMut(&str),
) -> Result<(), AnyError> {
  for entry in std::fs::read_dir(dir)? {
    let entry = entry?;
    let ty = entry.file_type()?;
    if ty.is_symlink() {
      let target = std::fs::read_link(entry.path())?;
      let name = node_modules_package_actual_dir_to_name(&target);
      if let Some(name) = name
        && !keep_names.contains(&*name)
      {
        if dry_run {
          #[allow(clippy::print_stderr)]
          {
            eprintln!(" {}", entry.path().display());
          }
        } else {
          on_remove(&name);
          cleaner.remove_file(&entry.path(), None)?;
        }
      }
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use super::Found::*;

  #[test]
  fn path_trie() {
    let mut trie = super::PathTrie::new();

    #[cfg(unix)]
    {
      trie.add_rewrite(
        Path::new("/RewriteMe").into(),
        Path::new("/Actual").into(),
      );
    }
    #[cfg(windows)]
    {
      trie.add_rewrite(
        Path::new("C:/RewriteMe").into(),
        Path::new("C:/Actual").into(),
      );
    }

    let paths = {
      #[cfg(unix)]
      {
        [
          "/foo/bar/deno",
          "/foo/bar/deno/1",
          "/foo/bar/deno/2",
          "/foo/baz",
          "/Actual/thing/quux",
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
          r"C:\Actual\thing\quux",
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
          ("/Actual/thing/quux", Some(Match)),
          ("/RewriteMe/thing/quux", Some(Match)),
          ("/RewriteMe/thing", Some(Prefix)),
        ]
      }
      #[cfg(windows)]
      {
        [
          (r"C:\", Some(Prefix)),
          (r"C:\foo", Some(Prefix)),
          (r"C:\foo\", Some(Prefix)),
          (r"C:\foo\", Some(Prefix)),
          (r"C:\foo\bar", Some(Prefix)),
          (r"C:\foo\bar\deno\1", Some(Match)),
          (r"C:\foo\bar\deno\2", Some(Match)),
          (r"C:\foo\baz", Some(Match)),
          (r"C:\fo", None),
          (r"C:\foo\baz\deno", None),
          (r"D:\", Some(Prefix)),
          (r"E:\", None),
          (r"C:\Actual\thing\quux", Some(Match)),
          (r"C:\RewriteMe\thing\quux", Some(Match)),
          (r"C:\RewriteMe\thing", Some(Prefix)),
        ]
      }
    };

    for pth in paths {
      let path = Path::new(pth);
      trie.insert(path.into());
    }

    for (input, expect) in cases {
      let path = Path::new(input);
      assert_eq!(trie.find(path), expect, "on input: {input}");
    }
  }
}
