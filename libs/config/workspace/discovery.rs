// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use deno_maybe_sync::new_rc;
use deno_package_json::PackageJson;
use deno_package_json::PackageJsonLoadError;
use deno_package_json::PackageJsonRc;
use deno_path_util::url_from_directory_path;
use deno_path_util::url_from_file_path;
use deno_path_util::url_parent;
use deno_path_util::url_to_file_path;
use indexmap::IndexSet;
use sys_traits::FsMetadata;
use sys_traits::FsRead;
use sys_traits::FsReadDir;
use url::Url;

use super::ResolveWorkspaceLinkError;
use super::ResolveWorkspaceLinkErrorKind;
use super::ResolveWorkspaceMemberError;
use super::ResolveWorkspaceMemberErrorKind;
use super::UrlRc;
use super::VendorEnablement;
use super::WorkspaceDiscoverError;
use super::WorkspaceDiscoverErrorKind;
use super::WorkspaceDiscoverOptions;
use super::WorkspaceDiscoverStart;
use super::WorkspaceRc;
use crate::deno_json::ConfigFile;
use crate::deno_json::ConfigFileRc;
use crate::glob::FileCollector;
use crate::glob::FilePatterns;
use crate::glob::PathOrPattern;
use crate::glob::PathOrPatternSet;
use crate::glob::is_glob_pattern;
use crate::util::is_skippable_io_error;
use crate::workspace::ConfigReadError;
use crate::workspace::Workspace;

#[derive(Debug)]
pub enum DenoOrPkgJson {
  Deno(ConfigFileRc),
  PkgJson(PackageJsonRc),
}

impl DenoOrPkgJson {
  pub fn specifier(&self) -> Cow<'_, Url> {
    match self {
      Self::Deno(config) => Cow::Borrowed(&config.specifier),
      Self::PkgJson(pkg_json) => Cow::Owned(pkg_json.specifier()),
    }
  }
}

#[derive(Debug)]
pub enum ConfigFolder {
  Single(DenoOrPkgJson),
  Both {
    deno_json: ConfigFileRc,
    pkg_json: PackageJsonRc,
  },
}

impl ConfigFolder {
  pub fn folder_url(&self) -> Url {
    match self {
      Self::Single(DenoOrPkgJson::Deno(config)) => {
        url_parent(&config.specifier)
      }
      Self::Single(DenoOrPkgJson::PkgJson(pkg_json)) => {
        url_from_directory_path(pkg_json.path.parent().unwrap()).unwrap()
      }
      Self::Both { deno_json, .. } => url_parent(&deno_json.specifier),
    }
  }

  pub fn has_workspace_members(&self) -> bool {
    match self {
      Self::Single(DenoOrPkgJson::Deno(config)) => {
        config.json.workspace.is_some()
      }
      Self::Single(DenoOrPkgJson::PkgJson(pkg_json)) => {
        pkg_json.workspaces.is_some()
      }
      Self::Both {
        deno_json,
        pkg_json,
      } => deno_json.json.workspace.is_some() || pkg_json.workspaces.is_some(),
    }
  }

  pub fn deno_json(&self) -> Option<&ConfigFileRc> {
    match self {
      Self::Single(DenoOrPkgJson::Deno(deno_json)) => Some(deno_json),
      Self::Both { deno_json, .. } => Some(deno_json),
      _ => None,
    }
  }

  pub fn pkg_json(&self) -> Option<&PackageJsonRc> {
    match self {
      Self::Single(DenoOrPkgJson::PkgJson(pkg_json)) => Some(pkg_json),
      Self::Both { pkg_json, .. } => Some(pkg_json),
      _ => None,
    }
  }

  pub fn from_maybe_both(
    maybe_deno_json: Option<ConfigFileRc>,
    maybe_pkg_json: Option<PackageJsonRc>,
  ) -> Option<Self> {
    match (maybe_deno_json, maybe_pkg_json) {
      (Some(deno_json), Some(pkg_json)) => Some(Self::Both {
        deno_json,
        pkg_json,
      }),
      (Some(deno_json), None) => {
        Some(Self::Single(DenoOrPkgJson::Deno(deno_json)))
      }
      (None, Some(pkg_json)) => {
        Some(Self::Single(DenoOrPkgJson::PkgJson(pkg_json)))
      }
      (None, None) => None,
    }
  }
}

#[derive(Debug)]
pub enum ConfigFileDiscovery {
  None { maybe_vendor_dir: Option<PathBuf> },
  Workspace { workspace: WorkspaceRc },
}

impl ConfigFileDiscovery {
  fn root_config_specifier(&self) -> Option<Cow<'_, Url>> {
    match self {
      Self::None { .. } => None,
      Self::Workspace { workspace, .. } => {
        let root_folder_configs = workspace.root_folder_configs();
        if let Some(deno_json) = &root_folder_configs.deno_json {
          return Some(Cow::Borrowed(&deno_json.specifier));
        }
        if let Some(pkg_json) = &root_folder_configs.pkg_json {
          return Some(Cow::Owned(pkg_json.specifier()));
        }
        None
      }
    }
  }
}

fn config_folder_config_specifier(res: &ConfigFolder) -> Cow<'_, Url> {
  match res {
    ConfigFolder::Single(config) => config.specifier(),
    ConfigFolder::Both { deno_json, .. } => Cow::Borrowed(&deno_json.specifier),
  }
}

pub fn discover_workspace_config_files<
  TSys: FsRead + FsMetadata + FsReadDir,
>(
  sys: &TSys,
  start: WorkspaceDiscoverStart,
  opts: &WorkspaceDiscoverOptions,
) -> Result<ConfigFileDiscovery, WorkspaceDiscoverError> {
  match start {
    WorkspaceDiscoverStart::Paths(dirs) => match dirs.len() {
      0 => Ok(ConfigFileDiscovery::None {
        maybe_vendor_dir: resolve_vendor_dir(
          None,
          opts.maybe_vendor_override.as_ref(),
        ),
      }),
      1 => {
        let dir = &dirs[0];
        let start = DirOrConfigFile::Dir(dir);
        discover_workspace_config_files_for_single_dir(sys, start, opts, None)
      }
      _ => {
        let mut checked = HashSet::default();
        let mut final_workspace = ConfigFileDiscovery::None {
          maybe_vendor_dir: resolve_vendor_dir(
            None,
            opts.maybe_vendor_override.as_ref(),
          ),
        };
        for dir in dirs {
          let workspace = discover_workspace_config_files_for_single_dir(
            sys,
            DirOrConfigFile::Dir(dir),
            opts,
            Some(&mut checked),
          )?;
          if let Some(root_config_specifier) = workspace.root_config_specifier()
          {
            if let Some(final_workspace_config_specifier) =
              final_workspace.root_config_specifier()
            {
              return Err(WorkspaceDiscoverError(
                WorkspaceDiscoverErrorKind::MultipleWorkspaces {
                  base_workspace_url: final_workspace_config_specifier
                    .into_owned(),
                  other_workspace_url: root_config_specifier.into_owned(),
                }
                .into(),
              ));
            }
            final_workspace = workspace;
          }
        }
        Ok(final_workspace)
      }
    },
    WorkspaceDiscoverStart::ConfigFile(file) => {
      let start = DirOrConfigFile::ConfigFile(file);
      discover_workspace_config_files_for_single_dir(sys, start, opts, None)
    }
  }
}

#[derive(Debug, Clone, Copy)]
enum DirOrConfigFile<'a> {
  Dir(&'a Path),
  ConfigFile(&'a Path),
}

fn discover_workspace_config_files_for_single_dir<
  TSys: FsRead + FsMetadata + FsReadDir,
>(
  sys: &TSys,
  start: DirOrConfigFile,
  opts: &WorkspaceDiscoverOptions,
  mut checked: Option<&mut HashSet<PathBuf>>,
) -> Result<ConfigFileDiscovery, WorkspaceDiscoverError> {
  fn strip_up_to_node_modules(path: &Path) -> PathBuf {
    path
      .components()
      .take_while(|component| match component {
        std::path::Component::Normal(name) => {
          name.to_string_lossy() != "node_modules"
        }
        _ => true,
      })
      .collect()
  }

  if opts.workspace_cache.is_some() {
    // it doesn't really make sense to use a workspace cache without config
    // caches because that would mean the configs might change between calls
    // causing strange behavior, so panic if someone does this
    assert!(
      opts.deno_json_cache.is_some() && opts.pkg_json_cache.is_some(),
      "Using a workspace cache requires setting the deno.json and package.json caches"
    );
  }

  let start_dir: Option<&Path>;
  let mut first_config_folder_url: Option<Url> = None;
  let mut found_config_folders: HashMap<_, ConfigFolder> = HashMap::new();
  let config_file_names =
    ConfigFile::resolve_config_file_names(opts.additional_config_file_names);
  let load_pkg_json_in_folder = |folder_path: &Path| {
    if opts.discover_pkg_json {
      let pkg_json_path = folder_path.join("package.json");
      match PackageJson::load_from_path(
        sys,
        opts.pkg_json_cache,
        &pkg_json_path,
      ) {
        Ok(pkg_json) => {
          if pkg_json.is_some() {
            log::debug!(
              "package.json file found at '{}'",
              pkg_json_path.display()
            );
          }
          Ok(pkg_json)
        }
        Err(PackageJsonLoadError::Io { source, .. })
          if is_skippable_io_error(&source) =>
        {
          Ok(None)
        }
        Err(err) => Err(err),
      }
    } else {
      Ok(None)
    }
  };
  let load_config_folder = |folder_path: &Path| -> Result<_, ConfigReadError> {
    let maybe_config_file = ConfigFile::maybe_find_in_folder(
      sys,
      opts.deno_json_cache,
      folder_path,
      &config_file_names,
    )?;
    let maybe_pkg_json = load_pkg_json_in_folder(folder_path)?;
    Ok(ConfigFolder::from_maybe_both(
      maybe_config_file,
      maybe_pkg_json,
    ))
  };
  match start {
    DirOrConfigFile::Dir(dir) => {
      start_dir = Some(dir);
    }
    DirOrConfigFile::ConfigFile(file) => {
      let specifier = url_from_file_path(file)?;
      let config_file = new_rc(
        ConfigFile::from_specifier(sys, specifier.clone())
          .map_err(ConfigReadError::DenoJsonRead)?,
      );

      // see what config would be loaded if we just specified the parent directory
      let natural_config_folder_result =
        load_config_folder(file.parent().unwrap());
      let matching_config_folder = match natural_config_folder_result {
        Ok(Some(natual_config_folder)) => {
          if natual_config_folder
            .deno_json()
            .is_some_and(|d| d.specifier == config_file.specifier)
          {
            Some(natual_config_folder)
          } else {
            None
          }
        }
        Ok(None) | Err(_) => None,
      };

      let parent_dir_url = url_parent(&config_file.specifier);
      let config_folder = match matching_config_folder {
        Some(config_folder) => config_folder,
        None => {
          // when loading the directory we would have loaded something else, so
          // don't try to load a workspace and don't store this information in
          // the workspace cache
          let config_folder =
            ConfigFolder::Single(DenoOrPkgJson::Deno(config_file));

          if config_folder.has_workspace_members() {
            return handle_workspace_folder_with_members(
              sys,
              config_folder,
              Some(&parent_dir_url),
              opts,
              found_config_folders,
              &load_config_folder,
            );
          }

          let maybe_vendor_dir = resolve_vendor_dir(
            config_folder.deno_json().map(|d| d.as_ref()),
            opts.maybe_vendor_override.as_ref(),
          );
          let links = resolve_link_config_folders(
            sys,
            &config_folder,
            load_config_folder,
          )?;
          return Ok(ConfigFileDiscovery::Workspace {
            workspace: new_rc(Workspace::new(
              config_folder,
              Default::default(),
              links,
              maybe_vendor_dir,
            )),
          });
        }
      };

      if let Some(workspace_cache) = &opts.workspace_cache
        && let Some(workspace) = workspace_cache.get(&config_file.dir_path())
      {
        if cfg!(debug_assertions) {
          let expected_vendor_dir = resolve_vendor_dir(
            config_folder.deno_json().map(|d| d.as_ref()),
            opts.maybe_vendor_override.as_ref(),
          );
          debug_assert_eq!(
            expected_vendor_dir, workspace.vendor_dir,
            "should not be using a different vendor dir across calls"
          );
        }
        return Ok(ConfigFileDiscovery::Workspace {
          workspace: workspace.clone(),
        });
      }

      if config_folder.has_workspace_members() {
        return handle_workspace_folder_with_members(
          sys,
          config_folder,
          Some(&parent_dir_url),
          opts,
          found_config_folders,
          &load_config_folder,
        );
      }

      found_config_folders.insert(parent_dir_url.clone(), config_folder);
      first_config_folder_url = Some(parent_dir_url);
      // start searching for a workspace in the parent directory
      start_dir = file.parent().and_then(|p| p.parent());
    }
  }
  // do not auto-discover inside the node_modules folder (ex. when a
  // user is running something directly within there)
  let start_dir = start_dir.map(strip_up_to_node_modules);
  for current_dir in start_dir.iter().flat_map(|p| p.ancestors()) {
    if let Some(checked) = checked.as_mut()
      && !checked.insert(current_dir.to_path_buf())
    {
      // already visited here, so exit
      return Ok(ConfigFileDiscovery::None {
        maybe_vendor_dir: resolve_vendor_dir(
          None,
          opts.maybe_vendor_override.as_ref(),
        ),
      });
    }

    if let Some(workspace_with_members) = opts
      .workspace_cache
      .and_then(|c| c.get(current_dir))
      .filter(|w| w.config_folders.len() > 1)
    {
      if cfg!(debug_assertions) {
        let expected_vendor_dir = resolve_vendor_dir(
          workspace_with_members.root_deno_json().map(|d| d.as_ref()),
          opts.maybe_vendor_override.as_ref(),
        );
        debug_assert_eq!(
          expected_vendor_dir, workspace_with_members.vendor_dir,
          "should not be using a different vendor dir across calls"
        );
      }

      return handle_workspace_with_members(
        sys,
        workspace_with_members,
        first_config_folder_url.as_ref(),
        found_config_folders,
        opts,
        load_config_folder,
      );
    }

    let maybe_config_folder = load_config_folder(current_dir)?;
    let Some(root_config_folder) = maybe_config_folder else {
      continue;
    };
    if root_config_folder.has_workspace_members() {
      return handle_workspace_folder_with_members(
        sys,
        root_config_folder,
        first_config_folder_url.as_ref(),
        opts,
        found_config_folders,
        &load_config_folder,
      );
    }

    let config_folder_url = root_config_folder.folder_url();
    if first_config_folder_url.is_none() {
      if let Some(workspace_cache) = &opts.workspace_cache
        && let Some(workspace) = workspace_cache.get(current_dir)
      {
        if cfg!(debug_assertions) {
          let expected_vendor_dir = resolve_vendor_dir(
            root_config_folder.deno_json().map(|d| d.as_ref()),
            opts.maybe_vendor_override.as_ref(),
          );
          debug_assert_eq!(
            expected_vendor_dir, workspace.vendor_dir,
            "should not be using a different vendor dir across calls"
          );
        }
        return Ok(ConfigFileDiscovery::Workspace {
          workspace: workspace.clone(),
        });
      }

      first_config_folder_url = Some(config_folder_url.clone());
    }
    found_config_folders.insert(config_folder_url, root_config_folder);
  }

  if let Some(first_config_folder_url) = first_config_folder_url {
    let config_folder = found_config_folders
      .remove(&first_config_folder_url)
      .unwrap();
    let maybe_vendor_dir = resolve_vendor_dir(
      config_folder.deno_json().map(|d| d.as_ref()),
      opts.maybe_vendor_override.as_ref(),
    );
    let link =
      resolve_link_config_folders(sys, &config_folder, load_config_folder)?;
    let workspace = new_rc(Workspace::new(
      config_folder,
      Default::default(),
      link,
      maybe_vendor_dir,
    ));
    if let Some(cache) = opts.workspace_cache {
      cache.set(workspace.root_dir_path(), workspace.clone());
    }
    Ok(ConfigFileDiscovery::Workspace { workspace })
  } else {
    Ok(ConfigFileDiscovery::None {
      maybe_vendor_dir: resolve_vendor_dir(
        None,
        opts.maybe_vendor_override.as_ref(),
      ),
    })
  }
}

fn handle_workspace_folder_with_members<
  TSys: FsRead + FsMetadata + FsReadDir,
>(
  sys: &TSys,
  root_config_folder: ConfigFolder,
  first_config_folder_url: Option<&Url>,
  opts: &WorkspaceDiscoverOptions<'_>,
  mut found_config_folders: HashMap<Url, ConfigFolder>,
  load_config_folder: &impl Fn(
    &Path,
  ) -> Result<Option<ConfigFolder>, ConfigReadError>,
) -> Result<ConfigFileDiscovery, WorkspaceDiscoverError> {
  let maybe_vendor_dir = resolve_vendor_dir(
    root_config_folder.deno_json().map(|d| d.as_ref()),
    opts.maybe_vendor_override.as_ref(),
  );
  let raw_root_workspace = resolve_workspace_for_config_folder(
    sys,
    root_config_folder,
    maybe_vendor_dir,
    &mut found_config_folders,
    load_config_folder,
  )?;
  let links = resolve_link_config_folders(
    sys,
    &raw_root_workspace.root,
    load_config_folder,
  )?;
  let root_workspace = new_rc(Workspace::new(
    raw_root_workspace.root,
    raw_root_workspace.members,
    links,
    raw_root_workspace.vendor_dir,
  ));
  if let Some(cache) = opts.workspace_cache {
    cache.set(root_workspace.root_dir_path(), root_workspace.clone());
  }
  handle_workspace_with_members(
    sys,
    root_workspace,
    first_config_folder_url,
    found_config_folders,
    opts,
    load_config_folder,
  )
}

fn handle_workspace_with_members<TSys: FsRead + FsMetadata + FsReadDir>(
  sys: &TSys,
  root_workspace: WorkspaceRc,
  first_config_folder_url: Option<&Url>,
  mut found_config_folders: HashMap<Url, ConfigFolder>,
  opts: &WorkspaceDiscoverOptions,
  load_config_folder: impl Fn(
    &Path,
  ) -> Result<Option<ConfigFolder>, ConfigReadError>,
) -> Result<ConfigFileDiscovery, WorkspaceDiscoverError> {
  let is_root_deno_json_workspace = root_workspace
    .root_deno_json()
    .map(|d| d.json.workspace.is_some())
    .unwrap_or(false);
  // if the root was an npm workspace that doesn't have the start config
  // as a member then only resolve the start config
  if !is_root_deno_json_workspace
    && let Some(first_config_folder) = &first_config_folder_url
    && !root_workspace
      .config_folders
      .contains_key(*first_config_folder)
    && let Some(config_folder) =
      found_config_folders.remove(first_config_folder)
  {
    let maybe_vendor_dir = resolve_vendor_dir(
      config_folder.deno_json().map(|d| d.as_ref()),
      opts.maybe_vendor_override.as_ref(),
    );
    let links =
      resolve_link_config_folders(sys, &config_folder, load_config_folder)?;
    let workspace = new_rc(Workspace::new(
      config_folder,
      Default::default(),
      links,
      maybe_vendor_dir,
    ));
    if let Some(cache) = opts.workspace_cache {
      cache.set(workspace.root_dir_path(), workspace.clone());
    }
    return Ok(ConfigFileDiscovery::Workspace { workspace });
  }

  if is_root_deno_json_workspace {
    for (key, config_folder) in &found_config_folders {
      if !root_workspace.config_folders.contains_key(key) {
        return Err(
          WorkspaceDiscoverErrorKind::ConfigNotWorkspaceMember {
            workspace_url: (**root_workspace.root_dir_url()).clone(),
            config_url: config_folder_config_specifier(config_folder)
              .into_owned(),
          }
          .into(),
        );
      }
    }
  }

  // ensure no duplicate names in deno configuration files
  let mut seen_names: HashMap<&str, &Url> =
    HashMap::with_capacity(root_workspace.config_folders.len() + 1);
  for deno_json in root_workspace.deno_jsons() {
    if let Some(name) = deno_json.json.name.as_deref() {
      if let Some(other_member_url) = seen_names.get(name) {
        return Err(
          ResolveWorkspaceMemberErrorKind::DuplicatePackageName {
            name: name.to_string(),
            deno_json_url: deno_json.specifier.clone(),
            other_deno_json_url: (*other_member_url).clone(),
          }
          .into_box()
          .into(),
        );
      } else {
        seen_names.insert(name, &deno_json.specifier);
      }
    }
  }

  Ok(ConfigFileDiscovery::Workspace {
    workspace: root_workspace,
  })
}

struct RawResolvedWorkspace {
  root: ConfigFolder,
  members: BTreeMap<UrlRc, ConfigFolder>,
  vendor_dir: Option<PathBuf>,
}

fn resolve_workspace_for_config_folder<
  TSys: FsRead + FsMetadata + FsReadDir,
>(
  sys: &TSys,
  root_config_folder: ConfigFolder,
  maybe_vendor_dir: Option<PathBuf>,
  found_config_folders: &mut HashMap<Url, ConfigFolder>,
  load_config_folder: impl Fn(
    &Path,
  ) -> Result<Option<ConfigFolder>, ConfigReadError>,
) -> Result<RawResolvedWorkspace, WorkspaceDiscoverError> {
  let mut final_members = BTreeMap::new();
  let root_config_file_directory_url = root_config_folder.folder_url();
  let resolve_member_url =
    |raw_member: &str| -> Result<Url, ResolveWorkspaceMemberError> {
      let member = ensure_trailing_slash(raw_member);
      let member_dir_url = root_config_file_directory_url
        .join(&member)
        .map_err(|err| {
          ResolveWorkspaceMemberErrorKind::InvalidMember {
            base: root_config_folder.folder_url(),
            member: raw_member.to_owned(),
            source: err,
          }
          .into_box()
        })?;
      Ok(member_dir_url)
    };
  let validate_member_url_is_descendant =
    |member_dir_url: &Url| -> Result<(), ResolveWorkspaceMemberError> {
      if !member_dir_url
        .as_str()
        .starts_with(root_config_file_directory_url.as_str())
      {
        return Err(
          ResolveWorkspaceMemberErrorKind::NonDescendant {
            workspace_url: root_config_file_directory_url.clone(),
            member_url: member_dir_url.clone(),
          }
          .into_box(),
        );
      }
      Ok(())
    };
  let mut find_member_config_folder =
    |member_dir_url: &Url| -> Result<_, ResolveWorkspaceMemberError> {
      // try to find the config folder in memory from the configs we already
      // found on the file system
      if let Some(config_folder) = found_config_folders.remove(member_dir_url) {
        return Ok(config_folder);
      }

      let maybe_config_folder =
        load_config_folder(&url_to_file_path(member_dir_url)?)?;
      maybe_config_folder.ok_or_else(|| {
        // it's fine this doesn't use all the possible config file names
        // as this is only used to enhance the error message
        if member_dir_url.as_str().ends_with("/deno.json/")
          || member_dir_url.as_str().ends_with("/deno.jsonc/")
          || member_dir_url.as_str().ends_with("/package.json/")
        {
          ResolveWorkspaceMemberErrorKind::NotFoundMaybeSpecifiedFile {
            dir_url: member_dir_url.clone(),
          }
          .into_box()
        } else {
          ResolveWorkspaceMemberErrorKind::NotFound {
            dir_url: member_dir_url.clone(),
          }
          .into_box()
        }
      })
    };

  let collect_member_config_folders =
    |kind: &'static str,
     pattern_members: Vec<&String>,
     dir_path: &Path,
     config_file_names: &'static [&'static str]|
     -> Result<Vec<PathBuf>, WorkspaceDiscoverErrorKind> {
      let patterns = pattern_members
        .iter()
        .flat_map(|raw_member| {
          config_file_names.iter().map(|config_file_name| {
            PathOrPattern::from_relative(
              dir_path,
              &format!(
                "{}{}",
                ensure_trailing_slash(raw_member),
                config_file_name
              ),
            )
            .map_err(|err| {
              ResolveWorkspaceMemberErrorKind::MemberToPattern {
                kind,
                base: root_config_file_directory_url.clone(),
                member: raw_member.to_string(),
                source: err,
              }
              .into_box()
            })
          })
        })
        .collect::<Result<Vec<_>, _>>()?;

      let paths = if patterns.is_empty() {
        Vec::new()
      } else {
        FileCollector::new(|_| true)
          .ignore_git_folder()
          .ignore_node_modules()
          .set_vendor_folder(maybe_vendor_dir.clone())
          .collect_file_patterns(
            sys,
            &FilePatterns {
              base: dir_path.to_path_buf(),
              include: Some(PathOrPatternSet::new(patterns)),
              exclude: PathOrPatternSet::new(Vec::new()),
            },
          )
      };

      Ok(paths)
    };

  if let Some(deno_json) = root_config_folder.deno_json()
    && let Some(workspace_config) = deno_json.to_workspace_config()?
  {
    let (pattern_members, path_members): (Vec<_>, Vec<_>) = workspace_config
      .members
      .iter()
      .partition(|member| is_glob_pattern(member) || member.starts_with('!'));

    // Deno workspaces can discover wildcard members that use either `deno.json`, `deno.jsonc` or `package.json`.
    // But it only works for Deno workspaces, npm workspaces don't discover `deno.json(c)` files, otherwise
    // we'd be incompatible with npm workspaces if we discovered more files.
    let deno_json_paths = collect_member_config_folders(
      "Deno",
      pattern_members,
      &deno_json.dir_path(),
      &["deno.json", "deno.jsonc", "package.json"],
    )?;

    let mut member_dir_urls =
      IndexSet::with_capacity(path_members.len() + deno_json_paths.len());
    for path_member in path_members {
      let member_dir_url = resolve_member_url(path_member)?;
      member_dir_urls.insert((path_member.clone(), member_dir_url));
    }
    for deno_json_path in deno_json_paths {
      let member_dir_url =
        url_from_directory_path(deno_json_path.parent().unwrap()).unwrap();
      member_dir_urls.insert((
        deno_json_path
          .parent()
          .unwrap()
          .to_string_lossy()
          .into_owned(),
        member_dir_url,
      ));
    }

    for (raw_member, member_dir_url) in member_dir_urls {
      if member_dir_url == root_config_file_directory_url {
        return Err(
          ResolveWorkspaceMemberErrorKind::InvalidSelfReference {
            member: raw_member.to_string(),
          }
          .into_box()
          .into(),
        );
      }
      validate_member_url_is_descendant(&member_dir_url)?;
      let member_config_folder = find_member_config_folder(&member_dir_url)?;
      let previous_member = final_members
        .insert(new_rc(member_dir_url.clone()), member_config_folder);
      if previous_member.is_some() {
        return Err(
          ResolveWorkspaceMemberErrorKind::Duplicate {
            member: raw_member.to_string(),
          }
          .into_box()
          .into(),
        );
      }
    }
  }
  if let Some(pkg_json) = root_config_folder.pkg_json()
    && let Some(members) = &pkg_json.workspaces
  {
    let (pattern_members, path_members): (Vec<_>, Vec<_>) = members
      .iter()
      .partition(|member| is_glob_pattern(member) || member.starts_with('!'));

    // npm workspaces can discover wildcard members `package.json` files, but not `deno.json(c)` files, otherwise
    // we'd be incompatible with npm workspaces if we discovered more files than just `package.json`.
    let pkg_json_paths = collect_member_config_folders(
      "npm",
      pattern_members,
      pkg_json.dir_path(),
      &["package.json"],
    )?;

    let mut member_dir_urls =
      IndexSet::with_capacity(path_members.len() + pkg_json_paths.len());
    for path_member in path_members {
      let member_dir_url = resolve_member_url(path_member)?;
      member_dir_urls.insert(member_dir_url);
    }
    for pkg_json_path in pkg_json_paths {
      let member_dir_url =
        url_from_directory_path(pkg_json_path.parent().unwrap())?;
      member_dir_urls.insert(member_dir_url);
    }

    for member_dir_url in member_dir_urls {
      if member_dir_url == root_config_file_directory_url {
        continue; // ignore self references
      }
      validate_member_url_is_descendant(&member_dir_url)?;
      let member_config_folder =
        match find_member_config_folder(&member_dir_url) {
          Ok(config_folder) => config_folder,
          Err(err) => {
            return Err(
              match err.into_kind() {
                ResolveWorkspaceMemberErrorKind::NotFound { dir_url } => {
                  // enhance the error to say we didn't find a package.json
                  ResolveWorkspaceMemberErrorKind::NotFoundPackageJson {
                    dir_url,
                  }
                  .into_box()
                }
                err => err.into_box(),
              }
              .into(),
            );
          }
        };
      if member_config_folder.pkg_json().is_none() {
        return Err(
          ResolveWorkspaceMemberErrorKind::NotFoundPackageJson {
            dir_url: member_dir_url,
          }
          .into_box()
          .into(),
        );
      }
      // don't surface errors about duplicate members for
      // package.json workspace members
      final_members.insert(new_rc(member_dir_url), member_config_folder);
    }
  }

  Ok(RawResolvedWorkspace {
    root: root_config_folder,
    members: final_members,
    vendor_dir: maybe_vendor_dir,
  })
}

fn resolve_link_config_folders<TSys: FsRead + FsMetadata + FsReadDir>(
  sys: &TSys,
  root_config_folder: &ConfigFolder,
  load_config_folder: impl Fn(
    &Path,
  ) -> Result<Option<ConfigFolder>, ConfigReadError>,
) -> Result<BTreeMap<UrlRc, ConfigFolder>, WorkspaceDiscoverError> {
  let Some(workspace_deno_json) = root_config_folder.deno_json() else {
    return Ok(Default::default());
  };
  let Some(link_members) = workspace_deno_json.to_link_config()? else {
    return Ok(Default::default());
  };
  let root_config_file_directory_url = root_config_folder.folder_url();
  let resolve_link_dir_url =
    |raw_link: &str| -> Result<Url, WorkspaceDiscoverError> {
      let link = ensure_trailing_slash(raw_link);
      // support someone specifying an absolute path
      if (!cfg!(windows) && link.starts_with('/')
        || cfg!(windows) && link.chars().any(|c| c == '\\'))
        && let Ok(value) =
          deno_path_util::url_from_file_path(Path::new(link.as_ref()))
      {
        return Ok(value);
      }
      let link_dir_url =
        root_config_file_directory_url.join(&link).map_err(|err| {
          WorkspaceDiscoverErrorKind::ResolveLink {
            base: root_config_file_directory_url.clone(),
            link: raw_link.to_owned(),
            source: err.into(),
          }
        })?;
      Ok(link_dir_url)
    };
  let mut final_config_folders = BTreeMap::new();
  for raw_member in &link_members {
    let link_dir_url = resolve_link_dir_url(raw_member)?;
    let link_configs = resolve_link_member_config_folders(
      sys,
      &link_dir_url,
      &load_config_folder,
    )
    .map_err(|err| WorkspaceDiscoverErrorKind::ResolveLink {
      base: root_config_file_directory_url.clone(),
      link: raw_member.to_string(),
      source: err,
    })?;

    for link_config_url in link_configs.keys() {
      if *link_config_url.as_ref() == root_config_file_directory_url {
        return Err(WorkspaceDiscoverError(
          WorkspaceDiscoverErrorKind::ResolveLink {
            base: root_config_file_directory_url.clone(),
            link: raw_member.to_string(),
            source: ResolveWorkspaceLinkErrorKind::WorkspaceMemberNotAllowed
              .into_box(),
          }
          .into(),
        ));
      }
    }

    final_config_folders.extend(link_configs);
  }

  Ok(final_config_folders)
}

fn resolve_link_member_config_folders<TSys: FsRead + FsMetadata + FsReadDir>(
  sys: &TSys,
  link_dir_url: &Url,
  load_config_folder: impl Fn(
    &Path,
  ) -> Result<Option<ConfigFolder>, ConfigReadError>,
) -> Result<BTreeMap<UrlRc, ConfigFolder>, ResolveWorkspaceLinkError> {
  let link_dir_path = url_to_file_path(link_dir_url)?;
  let maybe_config_folder = load_config_folder(&link_dir_path)?;
  let Some(config_folder) = maybe_config_folder else {
    return Err(
      ResolveWorkspaceLinkErrorKind::NotFound {
        dir_url: link_dir_url.clone(),
      }
      .into_box(),
    );
  };
  if config_folder.has_workspace_members() {
    let maybe_vendor_dir =
      resolve_vendor_dir(config_folder.deno_json().map(|d| d.as_ref()), None);
    let mut raw_workspace = resolve_workspace_for_config_folder(
      sys,
      config_folder,
      maybe_vendor_dir,
      &mut HashMap::new(),
      &load_config_folder,
    )
    .map_err(|err| ResolveWorkspaceLinkErrorKind::Workspace(Box::new(err)))?;
    raw_workspace
      .members
      .insert(new_rc(raw_workspace.root.folder_url()), raw_workspace.root);
    Ok(raw_workspace.members)
  } else {
    // attempt to find the root workspace directory
    for ancestor in link_dir_path.ancestors().skip(1) {
      let Ok(Some(config_folder)) = load_config_folder(ancestor) else {
        continue;
      };
      if config_folder.has_workspace_members() {
        let maybe_vendor_dir = resolve_vendor_dir(
          config_folder.deno_json().map(|d| d.as_ref()),
          None,
        );
        let Ok(mut raw_workspace) = resolve_workspace_for_config_folder(
          sys,
          config_folder,
          maybe_vendor_dir,
          &mut HashMap::new(),
          &load_config_folder,
        ) else {
          continue;
        };
        if raw_workspace.members.contains_key(link_dir_url) {
          raw_workspace.members.insert(
            new_rc(raw_workspace.root.folder_url()),
            raw_workspace.root,
          );
          return Ok(raw_workspace.members);
        }
      }
    }
    Ok(BTreeMap::from([(
      new_rc(link_dir_url.clone()),
      config_folder,
    )]))
  }
}

fn resolve_vendor_dir(
  maybe_deno_json: Option<&ConfigFile>,
  maybe_vendor_override: Option<&VendorEnablement>,
) -> Option<PathBuf> {
  if let Some(vendor_folder_override) = maybe_vendor_override {
    match vendor_folder_override {
      VendorEnablement::Disable => None,
      VendorEnablement::Enable { cwd } => match maybe_deno_json {
        Some(c) => Some(c.dir_path().join("vendor")),
        None => Some(cwd.join("vendor")),
      },
    }
  } else {
    let deno_json = maybe_deno_json?;
    if deno_json.vendor() == Some(true) {
      Some(deno_json.dir_path().join("vendor"))
    } else {
      None
    }
  }
}

fn ensure_trailing_slash(path: &str) -> Cow<'_, str> {
  if !path.ends_with('/') {
    Cow::Owned(format!("{}/", path))
  } else {
    Cow::Borrowed(path)
  }
}
