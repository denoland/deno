// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;

use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmPackageExtraInfo;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;

#[derive(Default)]
pub struct BinEntries<'a> {
  /// Packages that have colliding bin names
  collisions: HashSet<&'a NpmPackageId>,
  seen_names: HashMap<String, &'a NpmPackageId>,
  /// The bin entries
  entries: Vec<(&'a NpmResolutionPackage, PathBuf, NpmPackageExtraInfo)>,
  sorted: bool,
}

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
  #[cfg(unix)]
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
  #[cfg(unix)]
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

impl<'a> BinEntries<'a> {
  pub fn new() -> Self {
    Self::default()
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
    if !self.entries.is_empty() && !bin_node_modules_dir_path.exists() {
      std::fs::create_dir_all(bin_node_modules_dir_path).map_err(|source| {
        BinEntriesError::Creating {
          path: bin_node_modules_dir_path.to_path_buf(),
          source,
        }
      })?;
    }

    self.for_each_entry(
      snapshot,
      |_package_path, _script| {
        #[cfg(unix)]
        {
          let path = _package_path.join(_script);
          make_executable_if_exists(&path)?;
        }
        Ok(())
      },
      |package, extra, package_path, name, script| {
        let outcome = set_up_bin_entry(
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

pub fn set_up_bin_entry<'a>(
  package: &'a NpmResolutionPackage,
  #[allow(unused_variables)] extra: &'a NpmPackageExtraInfo,
  bin_name: &'a str,
  #[allow(unused_variables)] bin_script: &str,
  #[allow(unused_variables)] package_path: &'a Path,
  bin_node_modules_dir_path: &Path,
) -> Result<EntrySetupOutcome<'a>, BinEntriesError> {
  #[cfg(windows)]
  {
    set_up_bin_shim(package, bin_name, bin_node_modules_dir_path)?;
    Ok(EntrySetupOutcome::Success)
  }
  #[cfg(unix)]
  {
    symlink_bin_entry(
      package,
      extra,
      bin_name,
      bin_script,
      package_path,
      bin_node_modules_dir_path,
    )
  }
}

#[cfg(windows)]
fn set_up_bin_shim(
  package: &NpmResolutionPackage,
  bin_name: &str,
  bin_node_modules_dir_path: &Path,
) -> Result<(), BinEntriesError> {
  use std::fs;
  let mut cmd_shim = bin_node_modules_dir_path.join(bin_name);

  cmd_shim.set_extension("cmd");
  let shim = format!("@deno run -A npm:{}/{bin_name} %*", package.id.nv);
  fs::write(&cmd_shim, shim).map_err(|err| BinEntriesError::SetUpBin {
    name: bin_name.to_string(),
    path: cmd_shim.clone(),
    source: Box::new(err.into()),
  })?;

  Ok(())
}

#[cfg(unix)]
/// Make the file at `path` executable if it exists.
/// Returns `true` if the file exists, `false` otherwise.
fn make_executable_if_exists(path: &Path) -> Result<bool, BinEntriesError> {
  use std::io;
  use std::os::unix::fs::PermissionsExt;
  let mut perms = match std::fs::metadata(path) {
    Ok(metadata) => metadata.permissions(),
    Err(err) => {
      if err.kind() == io::ErrorKind::NotFound {
        return Ok(false);
      }
      return Err(err.into());
    }
  };
  if perms.mode() & 0o111 == 0 {
    // if the original file is not executable, make it executable
    perms.set_mode(perms.mode() | 0o111);
    std::fs::set_permissions(path, perms).map_err(|source| {
      BinEntriesError::Permissions {
        path: path.to_path_buf(),
        source,
      }
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

#[cfg(unix)]
fn symlink_bin_entry<'a>(
  package: &'a NpmResolutionPackage,
  extra: &'a NpmPackageExtraInfo,
  bin_name: &'a str,
  bin_script: &str,
  package_path: &'a Path,
  bin_node_modules_dir_path: &Path,
) -> Result<EntrySetupOutcome<'a>, BinEntriesError> {
  use std::io;
  use std::os::unix::fs::symlink;
  let link = bin_node_modules_dir_path.join(bin_name);
  let original = package_path.join(bin_script);

  let found = make_executable_if_exists(&original).map_err(|source| {
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

  let original_relative =
    crate::util::path::relative_path(bin_node_modules_dir_path, &original)
      .unwrap_or(original);

  if let Err(err) = symlink(&original_relative, &link) {
    if err.kind() == io::ErrorKind::AlreadyExists {
      // remove and retry
      std::fs::remove_file(&link).map_err(|source| {
        BinEntriesError::RemoveBinSymlink {
          path: link.clone(),
          source,
        }
      })?;
      symlink(&original_relative, &link).map_err(|source| {
        BinEntriesError::SetUpBin {
          name: bin_name.to_string(),
          path: original_relative.to_path_buf(),
          source: Box::new(source.into()),
        }
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
