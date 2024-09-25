// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::npm::managed::NpmResolutionPackage;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;

#[derive(Default)]
pub struct BinEntries<'a> {
  /// Packages that have colliding bin names
  collisions: HashSet<&'a NpmPackageId>,
  seen_names: HashMap<&'a str, &'a NpmPackageId>,
  /// The bin entries
  entries: Vec<(&'a NpmResolutionPackage, PathBuf)>,
}

/// Returns the name of the default binary for the given package.
/// This is the package name without the organization (`@org/`), if any.
fn default_bin_name(package: &NpmResolutionPackage) -> &str {
  package
    .id
    .nv
    .name
    .rsplit_once('/')
    .map_or(package.id.nv.name.as_str(), |(_, name)| name)
}

impl<'a> BinEntries<'a> {
  pub fn new() -> Self {
    Self::default()
  }

  /// Add a new bin entry (package with a bin field)
  pub fn add(
    &mut self,
    package: &'a NpmResolutionPackage,
    package_path: PathBuf,
  ) {
    // check for a new collision, if we haven't already
    // found one
    match package.bin.as_ref().unwrap() {
      deno_npm::registry::NpmPackageVersionBinEntry::String(_) => {
        let bin_name = default_bin_name(package);

        if let Some(other) = self.seen_names.insert(bin_name, &package.id) {
          self.collisions.insert(&package.id);
          self.collisions.insert(other);
        }
      }
      deno_npm::registry::NpmPackageVersionBinEntry::Map(entries) => {
        for name in entries.keys() {
          if let Some(other) = self.seen_names.insert(name, &package.id) {
            self.collisions.insert(&package.id);
            self.collisions.insert(other);
          }
        }
      }
    }

    self.entries.push((package, package_path));
  }

  fn for_each_entry(
    &mut self,
    snapshot: &NpmResolutionSnapshot,
    mut already_seen: impl FnMut(
      &Path,
      &str, // bin script
    ) -> Result<(), AnyError>,
    mut new: impl FnMut(
      &NpmResolutionPackage,
      &Path,
      &str, // bin name
      &str, // bin script
    ) -> Result<(), AnyError>,
  ) -> Result<(), AnyError> {
    if !self.collisions.is_empty() {
      // walking the dependency tree to find out the depth of each package
      // is sort of expensive, so we only do it if there's a collision
      sort_by_depth(snapshot, &mut self.entries, &mut self.collisions);
    }

    let mut seen = HashSet::new();

    for (package, package_path) in &self.entries {
      if let Some(bin_entries) = &package.bin {
        match bin_entries {
          deno_npm::registry::NpmPackageVersionBinEntry::String(script) => {
            let name = default_bin_name(package);
            if !seen.insert(name) {
              already_seen(package_path, script)?;
              // we already set up a bin entry with this name
              continue;
            }
            new(package, package_path, name, script)?;
          }
          deno_npm::registry::NpmPackageVersionBinEntry::Map(entries) => {
            for (name, script) in entries {
              if !seen.insert(name) {
                already_seen(package_path, script)?;
                // we already set up a bin entry with this name
                continue;
              }
              new(package, package_path, name, script)?;
            }
          }
        }
      }
    }

    Ok(())
  }

  /// Collect the bin entries into a vec of (name, script path)
  pub fn into_bin_files(
    mut self,
    snapshot: &NpmResolutionSnapshot,
  ) -> Vec<(String, PathBuf)> {
    let mut bins = Vec::new();
    self
      .for_each_entry(
        snapshot,
        |_, _| Ok(()),
        |_, package_path, name, script| {
          bins.push((name.to_string(), package_path.join(script)));
          Ok(())
        },
      )
      .unwrap();
    bins
  }

  /// Finish setting up the bin entries, writing the necessary files
  /// to disk.
  pub fn finish(
    mut self,
    snapshot: &NpmResolutionSnapshot,
    bin_node_modules_dir_path: &Path,
  ) -> Result<(), AnyError> {
    if !self.entries.is_empty() && !bin_node_modules_dir_path.exists() {
      std::fs::create_dir_all(bin_node_modules_dir_path).with_context(
        || format!("Creating '{}'", bin_node_modules_dir_path.display()),
      )?;
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
      |package, package_path, name, script| {
        set_up_bin_entry(
          package,
          name,
          script,
          package_path,
          bin_node_modules_dir_path,
        )
      },
    )?;

    Ok(())
  }
}

// walk the dependency tree to find out the depth of each package
// that has a bin entry, then sort them by depth
fn sort_by_depth(
  snapshot: &NpmResolutionSnapshot,
  bin_entries: &mut [(&NpmResolutionPackage, PathBuf)],
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

  bin_entries.sort_by(|(a, _), (b, _)| {
    depths
      .get(&a.id)
      .unwrap_or(&u64::MAX)
      .cmp(depths.get(&b.id).unwrap_or(&u64::MAX))
      .then_with(|| a.id.nv.cmp(&b.id.nv).reverse())
  });
}

pub fn set_up_bin_entry(
  package: &NpmResolutionPackage,
  bin_name: &str,
  #[allow(unused_variables)] bin_script: &str,
  #[allow(unused_variables)] package_path: &Path,
  bin_node_modules_dir_path: &Path,
) -> Result<(), AnyError> {
  #[cfg(windows)]
  {
    set_up_bin_shim(package, bin_name, bin_node_modules_dir_path)?;
  }
  #[cfg(unix)]
  {
    symlink_bin_entry(
      package,
      bin_name,
      bin_script,
      package_path,
      bin_node_modules_dir_path,
    )?;
  }
  Ok(())
}

#[cfg(windows)]
fn set_up_bin_shim(
  package: &NpmResolutionPackage,
  bin_name: &str,
  bin_node_modules_dir_path: &Path,
) -> Result<(), AnyError> {
  use std::fs;
  let mut cmd_shim = bin_node_modules_dir_path.join(bin_name);

  cmd_shim.set_extension("cmd");
  let shim = format!("@deno run -A npm:{}/{bin_name} %*", package.id.nv);
  fs::write(&cmd_shim, shim).with_context(|| {
    format!("Can't set up '{}' bin at {}", bin_name, cmd_shim.display())
  })?;

  Ok(())
}

#[cfg(unix)]
/// Make the file at `path` executable if it exists.
/// Returns `true` if the file exists, `false` otherwise.
fn make_executable_if_exists(path: &Path) -> Result<bool, AnyError> {
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
    std::fs::set_permissions(path, perms).with_context(|| {
      format!("Setting permissions on '{}'", path.display())
    })?;
  }

  Ok(true)
}

#[cfg(unix)]
fn symlink_bin_entry(
  _package: &NpmResolutionPackage,
  bin_name: &str,
  bin_script: &str,
  package_path: &Path,
  bin_node_modules_dir_path: &Path,
) -> Result<(), AnyError> {
  use std::io;
  use std::os::unix::fs::symlink;
  let link = bin_node_modules_dir_path.join(bin_name);
  let original = package_path.join(bin_script);

  let found = make_executable_if_exists(&original).with_context(|| {
    format!("Can't set up '{}' bin at {}", bin_name, original.display())
  })?;
  if !found {
    log::warn!(
      "{} Trying to set up '{}' bin for \"{}\", but the entry point \"{}\" doesn't exist.",
      deno_terminal::colors::yellow("Warning"),
      bin_name,
      package_path.display(),
      original.display()
    );
    return Ok(());
  }

  let original_relative =
    crate::util::path::relative_path(bin_node_modules_dir_path, &original)
      .unwrap_or(original);

  if let Err(err) = symlink(&original_relative, &link) {
    if err.kind() == io::ErrorKind::AlreadyExists {
      // remove and retry
      std::fs::remove_file(&link).with_context(|| {
        format!(
          "Failed to remove existing bin symlink at {}",
          link.display()
        )
      })?;
      symlink(&original_relative, &link).with_context(|| {
        format!(
          "Can't set up '{}' bin at {}",
          bin_name,
          original_relative.display()
        )
      })?;
      return Ok(());
    }
    return Err(err).with_context(|| {
      format!(
        "Can't set up '{}' bin at {}",
        bin_name,
        original_relative.display()
      )
    });
  }

  Ok(())
}
