// Copyright 2018-2025 the Deno authors. MIT license.

mod windows_shim;

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;

use deno_npm::NpmPackageExtraInfo;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_npm::resolution::NpmResolutionSnapshot;
use sys_traits::FsCreateDirAll;
use sys_traits::FsFileMetadata;
use sys_traits::FsFileSetPermissions;
use sys_traits::FsMetadata;
use sys_traits::FsMetadataValue;
use sys_traits::FsOpen;
use sys_traits::FsReadLink;
use sys_traits::FsRemoveFile;
use sys_traits::FsSymlinkFile;
use sys_traits::FsWrite;

/// Returns the name of the default binary for the given package.
/// This is the package name without the organization (`@org/`), if any.
fn default_bin_name(package: &NpmResolutionPackage) -> &str {
  package
    .id
    .nv
    .name
    .as_str()
    .rsplit_once('/')
    .map(|(_, name)| name)
    .unwrap_or(package.id.nv.name.as_str())
}

pub fn warn_missing_entrypoint(
  bin_name: &str,
  package_path: &Path,
  entrypoint: &Path,
) {
  log::warn!(
    "{} Trying to set up '{}' bin for \"{}\", but the entry point \"{}\" doesn't exist.",
    deno_terminal::colors::yellow("Warning"),
    bin_name,
    package_path.display(),
    entrypoint.display()
  );
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum BinEntriesError {
  #[class(inherit)]
  #[error("Creating '{path}'")]
  Creating {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error("Setting permissions on '{path}'")]
  Permissions {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error("Can't set up '{name}' bin at {path}")]
  SetUpBin {
    name: String,
    path: PathBuf,
    #[source]
    #[inherit]
    source: Box<Self>,
  },
  #[class(inherit)]
  #[error("Setting permissions on '{path}'")]
  RemoveBinSymlink {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
}

pub struct BinEntries<'a, TSys: SetupBinEntrySys> {
  /// Packages that have colliding bin names
  collisions: HashSet<&'a NpmPackageId>,
  seen_names: HashMap<String, &'a NpmPackageId>,
  /// The bin entries
  entries: Vec<(&'a NpmResolutionPackage, PathBuf, NpmPackageExtraInfo)>,
  sorted: bool,
  sys: &'a TSys,
}

impl<'a, TSys: SetupBinEntrySys> BinEntries<'a, TSys> {
  pub fn new(sys: &'a TSys) -> Self {
    Self {
      collisions: Default::default(),
      seen_names: Default::default(),
      entries: Default::default(),
      sorted: false,
      sys,
    }
  }

  /// Add a new bin entry (package with a bin field)
  pub fn add<'b>(
    &mut self,
    package: &'a NpmResolutionPackage,
    extra: &'b NpmPackageExtraInfo,
    package_path: PathBuf,
  ) {
    self.sorted = false;
    // check for a new collision, if we haven't already
    // found one
    match extra.bin.as_ref().unwrap() {
      deno_npm::registry::NpmPackageVersionBinEntry::String(_) => {
        let bin_name = default_bin_name(package);

        if let Some(other) =
          self.seen_names.insert(bin_name.to_string(), &package.id)
        {
          self.collisions.insert(&package.id);
          self.collisions.insert(other);
        }
      }
      deno_npm::registry::NpmPackageVersionBinEntry::Map(entries) => {
        for name in entries.keys() {
          if let Some(other) =
            self.seen_names.insert(name.to_string(), &package.id)
          {
            self.collisions.insert(&package.id);
            self.collisions.insert(other);
          }
        }
      }
    }

    self.entries.push((package, package_path, extra.clone()));
  }

  fn for_each_entry(
    &mut self,
    snapshot: &NpmResolutionSnapshot,
    mut already_seen: impl FnMut(
      &Path,
      &str, // bin script
    ) -> Result<(), BinEntriesError>,
    mut new: impl FnMut(
      &NpmResolutionPackage,
      &NpmPackageExtraInfo,
      &Path,
      &str, // bin name
      &str, // bin script
    ) -> Result<(), BinEntriesError>,
    mut filter: impl FnMut(&NpmResolutionPackage) -> bool,
  ) -> Result<(), BinEntriesError> {
    if !self.collisions.is_empty() && !self.sorted {
      // walking the dependency tree to find out the depth of each package
      // is sort of expensive, so we only do it if there's a collision
      sort_by_depth(snapshot, &mut self.entries, &mut self.collisions);
      self.sorted = true;
    }

    let mut seen = HashSet::new();

    for (package, package_path, extra) in &self.entries {
      if !filter(package) {
        continue;
      }
      if let Some(bin_entries) = &extra.bin {
        match bin_entries {
          deno_npm::registry::NpmPackageVersionBinEntry::String(script) => {
            let name = default_bin_name(package);
            if !seen.insert(name) {
              already_seen(package_path, script)?;
              // we already set up a bin entry with this name
              continue;
            }
            new(package, extra, package_path, name, script)?;
          }
          deno_npm::registry::NpmPackageVersionBinEntry::Map(entries) => {
            for (name, script) in entries {
              if !seen.insert(name) {
                already_seen(package_path, script)?;
                // we already set up a bin entry with this name
                continue;
              }
              new(package, extra, package_path, name, script)?;
            }
          }
        }
      }
    }

    Ok(())
  }

  /// Collect the bin entries into a vec of (name, script path)
  pub fn collect_bin_files(
    &mut self,
    snapshot: &NpmResolutionSnapshot,
  ) -> Vec<(String, PathBuf)> {
    let mut bins = Vec::new();
    self
      .for_each_entry(
        snapshot,
        |_, _| Ok(()),
        |_, _, package_path, name, script| {
          bins.push((name.to_string(), package_path.join(script)));
          Ok(())
        },
        |_| true,
      )
      .unwrap();
    bins
  }

  fn set_up_entries_filtered(
    mut self,
    snapshot: &NpmResolutionSnapshot,
    bin_node_modules_dir_path: &Path,
    filter: impl FnMut(&NpmResolutionPackage) -> bool,
    mut handler: impl FnMut(&EntrySetupOutcome<'_>),
  ) -> Result<(), BinEntriesError> {
    if !self.entries.is_empty()
      && !self.sys.fs_exists_no_err(bin_node_modules_dir_path)
    {
      self
        .sys
        .fs_create_dir_all(bin_node_modules_dir_path)
        .map_err(|source| BinEntriesError::Creating {
          path: bin_node_modules_dir_path.to_path_buf(),
          source,
        })?;
    }

    self.for_each_entry(
      snapshot,
      |_package_path, _script| {
        if !sys_traits::impls::is_windows() {
          let path = _package_path.join(_script);
          make_executable_if_exists(self.sys, &path)?;
        }
        Ok(())
      },
      |package, extra, package_path, name, script| {
        let outcome = set_up_bin_entry(
          self.sys,
          package,
          extra,
          name,
          script,
          package_path,
          bin_node_modules_dir_path,
        )?;
        handler(&outcome);
        Ok(())
      },
      filter,
    )?;

    Ok(())
  }

  /// Finish setting up the bin entries, writing the necessary files
  /// to disk.
  pub fn finish(
    self,
    snapshot: &NpmResolutionSnapshot,
    bin_node_modules_dir_path: &Path,
    handler: impl FnMut(&EntrySetupOutcome<'_>),
  ) -> Result<(), BinEntriesError> {
    self.set_up_entries_filtered(
      snapshot,
      bin_node_modules_dir_path,
      |_| true,
      handler,
    )
  }

  /// Finish setting up the bin entries, writing the necessary files
  /// to disk.
  pub fn finish_only(
    self,
    snapshot: &NpmResolutionSnapshot,
    bin_node_modules_dir_path: &Path,
    handler: impl FnMut(&EntrySetupOutcome<'_>),
    only: &HashSet<&NpmPackageId>,
  ) -> Result<(), BinEntriesError> {
    self.set_up_entries_filtered(
      snapshot,
      bin_node_modules_dir_path,
      |package| only.contains(&package.id),
      handler,
    )
  }
}

// walk the dependency tree to find out the depth of each package
// that has a bin entry, then sort them by depth
fn sort_by_depth(
  snapshot: &NpmResolutionSnapshot,
  bin_entries: &mut [(&NpmResolutionPackage, PathBuf, NpmPackageExtraInfo)],
  collisions: &mut HashSet<&NpmPackageId>,
) {
  enum Entry<'a> {
    Pkg(&'a NpmPackageId),
    IncreaseDepth,
  }

  let mut seen = HashSet::new();
  let mut depths: HashMap<&NpmPackageId, u64> =
    HashMap::with_capacity(collisions.len());

  let mut queue = VecDeque::new();
  queue.extend(snapshot.top_level_packages().map(Entry::Pkg));
  seen.extend(snapshot.top_level_packages());
  queue.push_back(Entry::IncreaseDepth);

  let mut current_depth = 0u64;

  while let Some(entry) = queue.pop_front() {
    if collisions.is_empty() {
      break;
    }
    let id = match entry {
      Entry::Pkg(id) => id,
      Entry::IncreaseDepth => {
        current_depth += 1;
        if queue.is_empty() {
          break;
        }
        queue.push_back(Entry::IncreaseDepth);
        continue;
      }
    };
    if let Some(package) = snapshot.package_from_id(id) {
      if collisions.remove(&package.id) {
        depths.insert(&package.id, current_depth);
      }
      for dep in package.dependencies.values() {
        if seen.insert(dep) {
          queue.push_back(Entry::Pkg(dep));
        }
      }
    }
  }

  bin_entries.sort_by(|(a, _, _), (b, _, _)| {
    depths
      .get(&a.id)
      .unwrap_or(&u64::MAX)
      .cmp(depths.get(&b.id).unwrap_or(&u64::MAX))
      .then_with(|| a.id.nv.cmp(&b.id.nv).reverse())
  });
}

#[sys_traits::auto_impl]
pub trait SetupBinEntrySys:
  FsOpen
  + FsWrite
  + FsSymlinkFile
  + FsRemoveFile
  + FsCreateDirAll
  + FsMetadata
  + FsReadLink
{
}

pub fn set_up_bin_entry<'a>(
  sys: &impl SetupBinEntrySys,
  package: &'a NpmResolutionPackage,
  extra: &'a NpmPackageExtraInfo,
  bin_name: &'a str,
  bin_script: &str,
  package_path: &'a Path,
  bin_node_modules_dir_path: &Path,
) -> Result<EntrySetupOutcome<'a>, BinEntriesError> {
  if sys_traits::impls::is_windows() {
    windows_shim::set_up_bin_shim(
      sys,
      package,
      extra,
      bin_name,
      bin_script,
      package_path,
      bin_node_modules_dir_path,
    )?;
    Ok(EntrySetupOutcome::Success)
  } else {
    symlink_bin_entry(
      sys,
      package,
      extra,
      bin_name,
      bin_script,
      package_path,
      bin_node_modules_dir_path,
    )
  }
}

/// Make the file at `path` executable if it exists.
/// Returns `true` if the file exists, `false` otherwise.
fn make_executable_if_exists(
  sys: &impl FsOpen,
  path: &Path,
) -> Result<bool, BinEntriesError> {
  let mut open_options = sys_traits::OpenOptions::new();
  open_options.read = true;
  open_options.write = true;
  open_options.truncate = false; // ensure false
  let mut file = match sys.fs_open(path, &open_options) {
    Ok(file) => file,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
      return Ok(false);
    }
    Err(err) => return Err(err.into()),
  };
  let metadata = file.fs_file_metadata()?;
  let mode = metadata.mode()?;
  if mode & 0o111 == 0 {
    // if the original file is not executable, make it executable
    file
      .fs_file_set_permissions(mode | 0o111)
      .map_err(|source| BinEntriesError::Permissions {
        path: path.to_path_buf(),
        source,
      })?;
  }

  Ok(true)
}

pub enum EntrySetupOutcome<'a> {
  #[cfg_attr(windows, allow(dead_code))]
  MissingEntrypoint {
    bin_name: &'a str,
    package_path: &'a Path,
    entrypoint: PathBuf,
    package: &'a NpmResolutionPackage,
    extra: &'a NpmPackageExtraInfo,
  },
  Success,
}

impl EntrySetupOutcome<'_> {
  pub fn warn_if_failed(&self) {
    match self {
      EntrySetupOutcome::MissingEntrypoint {
        bin_name,
        package_path,
        entrypoint,
        ..
      } => warn_missing_entrypoint(bin_name, package_path, entrypoint),
      EntrySetupOutcome::Success => {}
    }
  }
}

fn relative_path(from: &Path, to: &Path) -> Option<PathBuf> {
  pathdiff::diff_paths(to, from)
}

fn symlink_bin_entry<'a>(
  sys: &(impl FsOpen + FsSymlinkFile + FsRemoveFile + FsReadLink),
  package: &'a NpmResolutionPackage,
  extra: &'a NpmPackageExtraInfo,
  bin_name: &'a str,
  bin_script: &str,
  package_path: &'a Path,
  bin_node_modules_dir_path: &Path,
) -> Result<EntrySetupOutcome<'a>, BinEntriesError> {
  let link = bin_node_modules_dir_path.join(bin_name);
  let original = package_path.join(bin_script);

  let original_relative = relative_path(bin_node_modules_dir_path, &original)
    .map(Cow::Owned)
    .unwrap_or_else(|| Cow::Borrowed(&original));

  if let Ok(original_link) = sys.fs_read_link(&link)
    && *original_link == *original_relative
  {
    return Ok(EntrySetupOutcome::Success);
  }

  let found = make_executable_if_exists(sys, &original).map_err(|source| {
    BinEntriesError::SetUpBin {
      name: bin_name.to_string(),
      path: original.to_path_buf(),
      source: Box::new(source),
    }
  })?;
  if !found {
    return Ok(EntrySetupOutcome::MissingEntrypoint {
      bin_name,
      package_path,
      entrypoint: original,
      package,
      extra,
    });
  }

  if let Err(err) = sys.fs_symlink_file(&*original_relative, &link) {
    if err.kind() == std::io::ErrorKind::AlreadyExists {
      // remove and retry
      sys.fs_remove_file(&link).map_err(|source| {
        BinEntriesError::RemoveBinSymlink {
          path: link.clone(),
          source,
        }
      })?;
      sys
        .fs_symlink_file(&*original_relative, &link)
        .map_err(|source| BinEntriesError::SetUpBin {
          name: bin_name.to_string(),
          path: original_relative.to_path_buf(),
          source: Box::new(source.into()),
        })?;
      return Ok(EntrySetupOutcome::Success);
    }
    return Err(BinEntriesError::SetUpBin {
      name: bin_name.to_string(),
      path: original_relative.to_path_buf(),
      source: Box::new(err.into()),
    });
  }

  Ok(EntrySetupOutcome::Success)
}
