// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::npm::managed::NpmResolutionPackage;
use deno_core::error::AnyError;
use deno_core::{anyhow::Context, parking_lot::Mutex};
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use std::collections::{HashMap, VecDeque};
use std::{
  path::{Path, PathBuf},
  sync::atomic::AtomicBool,
};

#[derive(Default)]
// just to put them under a single mutex
struct NamesAndEntries {
  seen_names: std::collections::HashSet<String>,
  entries: Vec<(NpmResolutionPackage, PathBuf)>,
}

pub(super) struct BinEntries {
  has_collision: AtomicBool,
  entries: Mutex<NamesAndEntries>,
}

fn default_bin_name(package: &NpmResolutionPackage) -> &str {
  package
    .id
    .nv
    .name
    .rsplit_once('/')
    .map_or(package.id.nv.name.as_str(), |(_, name)| name)
}

impl BinEntries {
  pub(super) fn new() -> Self {
    Self {
      has_collision: AtomicBool::new(false),
      entries: Mutex::new(NamesAndEntries::default()),
    }
  }

  fn has_collision(&self) -> bool {
    self
      .has_collision
      .load(std::sync::atomic::Ordering::Relaxed)
  }

  pub(super) fn add(
    &self,
    package: NpmResolutionPackage,
    package_path: PathBuf,
  ) {
    let mut bin_entries = self.entries.lock();

    if !self.has_collision() {
      match package.bin.as_ref().unwrap() {
        deno_npm::registry::NpmPackageVersionBinEntry::String(_) => {
          let bin_name = default_bin_name(&package);
          if !bin_entries.seen_names.insert(bin_name.to_string()) {
            self
              .has_collision
              .store(true, std::sync::atomic::Ordering::Relaxed);
            bin_entries.seen_names.clear();
          }
        }
        deno_npm::registry::NpmPackageVersionBinEntry::Map(entries) => {
          for name in entries.keys() {
            if !bin_entries.seen_names.insert(name.clone()) {
              self
                .has_collision
                .store(true, std::sync::atomic::Ordering::Relaxed);
              bin_entries.seen_names.clear();
            }
          }
        }
      }
    }

    bin_entries.entries.push((package, package_path));
  }

  pub(super) fn finish(
    &self,
    snapshot: &NpmResolutionSnapshot,
    bin_node_modules_dir_path: &Path,
  ) -> Result<(), AnyError> {
    let mut bin_entries = self.entries.lock();

    if !bin_entries.entries.is_empty() && !bin_node_modules_dir_path.exists() {
      std::fs::create_dir_all(&bin_node_modules_dir_path).with_context(
        || format!("Creating '{}'", bin_node_modules_dir_path.display()),
      )?;
    }

    if self.has_collision() {
      sort_by_depth(snapshot, &mut bin_entries.entries);
    }

    let mut seen = std::collections::HashSet::new();

    for (package, package_path) in &*bin_entries.entries {
      if let Some(bin_entries) = &package.bin {
        match bin_entries {
          deno_npm::registry::NpmPackageVersionBinEntry::String(script) => {
            // the default bin name doesn't include the organization
            let name = default_bin_name(package);
            if !seen.insert(name) {
              continue;
            }
            set_up_bin_entry(
              package,
              name,
              script,
              package_path,
              &bin_node_modules_dir_path,
            )?;
          }
          deno_npm::registry::NpmPackageVersionBinEntry::Map(entries) => {
            for (name, script) in entries {
              if !seen.insert(name) {
                continue;
              }
              set_up_bin_entry(
                package,
                name,
                script,
                package_path,
                &bin_node_modules_dir_path,
              )?;
            }
          }
        }
      }
    }

    bin_entries.entries.clear();
    bin_entries.seen_names.clear();
    self
      .has_collision
      .store(false, std::sync::atomic::Ordering::Relaxed);

    Ok(())
  }
}

// walk the dependency tree to find out the depth of each package
// that has a bin entry, then sort them by depth
fn sort_by_depth(
  snapshot: &NpmResolutionSnapshot,
  bin_entries: &mut [(NpmResolutionPackage, PathBuf)],
) {
  enum Entry<'a> {
    Pkg(&'a NpmPackageId),
    IncreaseDepth,
  }
  let mut want = bin_entries
    .iter()
    .map(|(p, _)| p.id.clone())
    .collect::<std::collections::HashSet<_>>();
  let mut seen = std::collections::HashSet::new();
  let mut depths: HashMap<NpmPackageId, u64> =
    HashMap::with_capacity(want.len());

  let mut queue = VecDeque::new();
  queue.extend(snapshot.top_level_packages().map(Entry::Pkg));
  queue.push_back(Entry::IncreaseDepth);

  let mut current_depth = 0u64;

  while let Some(entry) = queue.pop_front() {
    if want.is_empty() {
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
      if want.remove(&package.id) {
        depths.insert(package.id.clone(), current_depth);
      }
      seen.insert(package.id.clone());
      queue.extend(
        package
          .dependencies
          .values()
          .filter(|p| !seen.contains(*p))
          .map(Entry::Pkg),
      );
    }
  }

  bin_entries.sort_by(|(a, _), (b, _)| {
    depths
      .get(&a.id)
      .unwrap_or_else(|| {
        log::warn!("{} not found in dependency tree", a.id.nv);
        &u64::MAX
      })
      .cmp(&depths.get(&b.id).unwrap_or_else(|| {
        log::warn!("{} not found in dependency tree", b.id.nv);
        &u64::MAX
      }))
      .then_with(|| a.id.nv.cmp(&b.id.nv).reverse())
  });
}

pub(super) fn set_up_bin_entry(
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
  if cmd_shim.exists() {
    if let Ok(contents) = fs::read_to_string(cmd_shim) {
      if contents == shim {
        // up to date
        return Ok(());
      }
    }
    return Ok(());
  }
  fs::write(&cmd_shim, shim).with_context(|| {
    format!("Can't set up '{}' bin at {}", bin_name, cmd_shim.display())
  })?;

  Ok(())
}

#[cfg(unix)]
fn symlink_bin_entry(
  _package: &NpmResolutionPackage,
  bin_name: &str,
  bin_script: &str,
  package_path: &Path,
  bin_node_modules_dir_path: &Path,
) -> Result<(), AnyError> {
  use std::os::unix::fs::symlink;
  let link = bin_node_modules_dir_path.join(bin_name);
  let original = package_path.join(bin_script);

  if !original.exists() {
    log::warn!(
      "{} Trying to set up '{}' bin for \"{}\", but the entry point \"{}\" doesn't exist.",
      deno_terminal::colors::yellow("Warning"),
      bin_name,
      package_path.display(),
      original.display()
    );
    return Ok(());
  }

  // Don't bother setting up another link if it already exists
  if link.exists() {
    let resolved = std::fs::read_link(&link).ok();
    if let Some(resolved) = resolved {
      if resolved != original {
        log::warn!(
          "{} Trying to set up '{}' bin for \"{}\", but an entry pointing to \"{}\" already exists. Skipping...", 
          deno_terminal::colors::yellow("Warning"), 
          bin_name,
          resolved.display(),
          original.display()
        );
      }
      return Ok(());
    }
  }

  use std::os::unix::fs::PermissionsExt;
  let mut perms = std::fs::metadata(&original).unwrap().permissions();
  if perms.mode() & 0o111 == 0 {
    // if the original file is not executable, make it executable
    perms.set_mode(perms.mode() | 0o111);
    std::fs::set_permissions(&original, perms).with_context(|| {
      format!("Setting permissions on '{}'", original.display())
    })?;
  }
  let original_relative =
    crate::util::path::relative_path(bin_node_modules_dir_path, &original)
      .unwrap_or(original);
  symlink(&original_relative, &link).with_context(|| {
    format!(
      "Can't set up '{}' bin at {}",
      bin_name,
      original_relative.display()
    )
  })?;

  Ok(())
}
