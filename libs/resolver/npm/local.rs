// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hasher;
use std::path::Path;

use deno_cache_dir::npm::mixed_case_package_name_decode;
use deno_cache_dir::npm::mixed_case_package_name_encode;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_npm::NpmSystemInfo;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_semver::StackString;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use sys_traits::FsDirEntry;

#[inline]
pub fn get_package_folder_id_folder_name(
  folder_id: &NpmPackageCacheFolderId,
) -> String {
  get_package_folder_id_folder_name_from_parts(
    &folder_id.nv,
    folder_id.copy_index,
  )
}

pub fn get_package_folder_id_folder_name_from_parts(
  nv: &PackageNv,
  copy_index: u8,
) -> String {
  let copy_str = if copy_index == 0 {
    Cow::Borrowed("")
  } else {
    Cow::Owned(format!("_{}", copy_index))
  };
  let name = normalize_pkg_name_for_node_modules_deno_folder(&nv.name);
  format!("{}@{}{}", name, nv.version, copy_str)
}

/// Gets a package folder name for the global virtual store.
///
/// Unlike `NpmPackageCacheFolderId`, this includes the full `NpmPackageId`,
/// which captures peer dependency resolution and is therefore safe to share
/// across projects with different peer graphs.
pub fn get_global_virtual_store_package_folder_name(
  id: &NpmPackageId,
  system_info: &NpmSystemInfo,
) -> String {
  let mut hasher = twox_hash::XxHash64::default();
  hasher.write(id.as_serialized().as_bytes());
  hasher.write(b"\0");
  hasher.write(system_info.os.as_bytes());
  hasher.write(b"\0");
  hasher.write(system_info.cpu.as_bytes());
  let hash = hasher.finish();
  let name = normalize_pkg_name_for_node_modules_deno_folder(&id.nv.name);
  format!("{}@{}_{:016x}", name, id.nv.version, hash)
}

pub fn get_global_virtual_store_patch_hash(
  sys: &(impl sys_traits::FsRead + sys_traits::FsReadDir),
  path: &Path,
) -> std::io::Result<String> {
  let mut hasher = twox_hash::XxHash64::default();
  write_hash_part(&mut hasher, "patch-v1");
  write_global_virtual_store_patch_dir_hash(sys, path, path, &mut hasher)?;
  Ok(format!("{:016x}", hasher.finish()))
}

#[derive(Debug, Clone, Default)]
pub enum GlobalVirtualStoreLifecycleScripts {
  All,
  Some(Vec<PackageReq>),
  #[default]
  None,
}

impl GlobalVirtualStoreLifecycleScripts {
  pub fn has_allowed_scripts(&self) -> bool {
    match self {
      Self::All => true,
      Self::Some(package_reqs) => !package_reqs.is_empty(),
      Self::None => false,
    }
  }

  pub fn can_run_scripts(&self, nv: &PackageNv) -> bool {
    match self {
      Self::All => true,
      Self::Some(package_reqs) => package_reqs
        .iter()
        .any(|req| req.name == nv.name && req.version_req.matches(&nv.version)),
      Self::None => false,
    }
  }

  pub fn merge(&mut self, other: Self) {
    match (self, other) {
      (Self::All, _) | (_, Self::None) => {}
      (this @ Self::None, other) => *this = other,
      (this @ Self::Some(_), Self::All) => *this = Self::All,
      (Self::Some(package_reqs), Self::Some(other_package_reqs)) => {
        package_reqs.extend(other_package_reqs);
      }
    }
  }
}

#[derive(Debug, Clone)]
pub struct GlobalVirtualStorePackageFolderNames {
  names: HashMap<NpmPackageId, String>,
}

impl GlobalVirtualStorePackageFolderNames {
  pub fn new(
    snapshot: &NpmResolutionSnapshot,
    system_info: &NpmSystemInfo,
    lifecycle_scripts: &GlobalVirtualStoreLifecycleScripts,
    patch_hashes: &HashMap<PackageNv, String>,
  ) -> Self {
    let packages = snapshot
      .all_packages_for_every_system()
      .map(|package| (&package.id, package))
      .collect::<HashMap<_, _>>();
    let mut graph_hashes = HashMap::with_capacity(packages.len());
    let mut build_taint = HashMap::with_capacity(packages.len());
    let mut names = HashMap::with_capacity(packages.len());
    for package in packages.values() {
      let graph_hash = graph_hash(
        &package.id,
        &packages,
        system_info,
        patch_hashes,
        &mut graph_hashes,
        &mut HashSet::new(),
      );
      let include_engine = transitively_requires_build(
        &package.id,
        &packages,
        system_info,
        lifecycle_scripts,
        &mut build_taint,
        &mut HashSet::new(),
      );
      let hash = package_folder_hash(&graph_hash, include_engine, system_info);
      let name =
        normalize_pkg_name_for_node_modules_deno_folder(&package.id.nv.name);
      names.insert(
        package.id.clone(),
        format!("{}@{}_{:016x}", name, package.id.nv.version, hash),
      );
    }
    Self { names }
  }

  pub fn get(&self, id: &NpmPackageId, system_info: &NpmSystemInfo) -> String {
    self.names.get(id).cloned().unwrap_or_else(|| {
      get_global_virtual_store_package_folder_name(id, system_info)
    })
  }
}

fn graph_hash(
  id: &NpmPackageId,
  packages: &HashMap<&NpmPackageId, &NpmResolutionPackage>,
  system_info: &NpmSystemInfo,
  patch_hashes: &HashMap<PackageNv, String>,
  cache: &mut HashMap<NpmPackageId, String>,
  parents: &mut HashSet<NpmPackageId>,
) -> String {
  if let Some(hash) = cache.get(id) {
    return hash.clone();
  }
  let Some(&package) = packages.get(id) else {
    return hash_str(&id.as_serialized());
  };
  if !parents.insert(id.clone()) {
    return hash_str(&id.as_serialized());
  }
  let mut hasher = twox_hash::XxHash64::default();
  write_hash_part(&mut hasher, "gvs-v2");
  write_hash_part(&mut hasher, &id.as_serialized());
  match &package.dist {
    Some(dist) => {
      write_hash_part(&mut hasher, &dist.tarball);
      let integrity_info = dist.integrity();
      let integrity = integrity_info
        .for_lockfile()
        .unwrap_or(Cow::Borrowed("no-integrity"));
      write_hash_part(&mut hasher, &integrity);
    }
    None => {
      write_hash_part(&mut hasher, "local");
    }
  }
  if let Some(hash) = patch_hashes.get(&id.nv) {
    write_hash_part(&mut hasher, "patch");
    write_hash_part(&mut hasher, hash);
  }

  let dependencies = package.dependencies.iter().collect::<BTreeMap<_, _>>();
  for (alias, dep_id) in dependencies {
    let Some(&dep) = packages.get(dep_id) else {
      continue;
    };
    if package.optional_dependencies.contains(alias)
      && !dep.system.matches_system(system_info)
    {
      continue;
    }
    let child_hash =
      graph_hash(dep_id, packages, system_info, patch_hashes, cache, parents);
    write_hash_part(&mut hasher, alias);
    write_hash_part(&mut hasher, &dep_id.as_serialized());
    write_hash_part(&mut hasher, &child_hash);
  }

  parents.remove(id);
  let hash = format!("{:016x}", hasher.finish());
  cache.insert(id.clone(), hash.clone());
  hash
}

fn transitively_requires_build(
  id: &NpmPackageId,
  packages: &HashMap<&NpmPackageId, &NpmResolutionPackage>,
  system_info: &NpmSystemInfo,
  lifecycle_scripts: &GlobalVirtualStoreLifecycleScripts,
  cache: &mut HashMap<NpmPackageId, bool>,
  parents: &mut HashSet<NpmPackageId>,
) -> bool {
  if let Some(requires_build) = cache.get(id) {
    return *requires_build;
  }
  let Some(&package) = packages.get(id) else {
    return false;
  };
  if !parents.insert(id.clone()) {
    return false;
  }
  let requires_build = (package.has_scripts
    && lifecycle_scripts.can_run_scripts(&id.nv))
    || package.dependencies.iter().any(|(alias, dep_id)| {
      let Some(&dep) = packages.get(dep_id) else {
        return false;
      };
      if package.optional_dependencies.contains(alias)
        && !dep.system.matches_system(system_info)
      {
        return false;
      }
      transitively_requires_build(
        dep_id,
        packages,
        system_info,
        lifecycle_scripts,
        cache,
        parents,
      )
    });
  parents.remove(id);
  cache.insert(id.clone(), requires_build);
  requires_build
}

fn package_folder_hash(
  graph_hash: &str,
  include_engine: bool,
  system_info: &NpmSystemInfo,
) -> u64 {
  let mut hasher = twox_hash::XxHash64::default();
  write_hash_part(&mut hasher, "gvs-node");
  write_hash_part(&mut hasher, graph_hash);
  if include_engine {
    write_hash_part(&mut hasher, "build");
    write_hash_part(&mut hasher, &system_info.os);
    write_hash_part(&mut hasher, &system_info.cpu);
  }
  hasher.finish()
}

fn hash_str(value: &str) -> String {
  let mut hasher = twox_hash::XxHash64::default();
  write_hash_part(&mut hasher, value);
  format!("{:016x}", hasher.finish())
}

fn write_hash_part(hasher: &mut impl Hasher, value: &str) {
  hasher.write(&value.len().to_le_bytes());
  hasher.write(value.as_bytes());
}

fn write_global_virtual_store_patch_dir_hash(
  sys: &(impl sys_traits::FsRead + sys_traits::FsReadDir),
  root_dir: &Path,
  dir: &Path,
  hasher: &mut impl Hasher,
) -> std::io::Result<()> {
  let mut entries = Vec::new();
  for entry in sys.fs_read_dir(dir)? {
    let entry = entry?;
    let file_name = entry.file_name();
    let file_name_str = file_name.to_string_lossy();
    if file_name_str == "node_modules" {
      continue;
    }
    entries.push((file_name_str.into_owned(), entry));
  }
  entries.sort_by(|a, b| a.0.cmp(&b.0));
  for (_, entry) in entries {
    let path = entry.path();
    let Ok(relative_path) = path.strip_prefix(root_dir) else {
      continue;
    };
    let relative_path = relative_path.to_string_lossy();
    write_hash_part(hasher, &relative_path);
    let file_type = entry.file_type()?;
    if file_type.is_dir() {
      write_hash_part(hasher, "d");
      write_global_virtual_store_patch_dir_hash(sys, root_dir, &path, hasher)?;
    } else if file_type.is_file() {
      write_hash_part(hasher, "f");
      let bytes = sys.fs_read(&path)?;
      hasher.write(&bytes.len().to_le_bytes());
      hasher.write(&bytes);
    }
  }
  Ok(())
}

pub fn get_package_folder_id_from_folder_name(
  folder_name: &str,
) -> Option<NpmPackageCacheFolderId> {
  let folder_name = folder_name.replace('+', "/");
  let (name, ending) = folder_name.rsplit_once('@')?;
  let name: StackString = if let Some(encoded_name) = name.strip_prefix('_') {
    StackString::from_string(mixed_case_package_name_decode(encoded_name)?)
  } else {
    name.into()
  };
  let (raw_version, copy_index) = match ending.split_once('_') {
    Some((raw_version, copy_index)) => {
      let copy_index = copy_index.parse::<u8>().ok()?;
      (raw_version, copy_index)
    }
    None => (ending, 0),
  };
  let version = deno_semver::Version::parse_from_npm(raw_version).ok()?;
  Some(NpmPackageCacheFolderId {
    nv: PackageNv { name, version },
    copy_index,
  })
}

/// Normalizes a package name for use at `node_modules/.deno/<pkg-name>@<version>[_<copy_index>]`
pub fn normalize_pkg_name_for_node_modules_deno_folder(
  name: &str,
) -> Cow<'_, str> {
  let name = if name.to_lowercase() == name {
    Cow::Borrowed(name)
  } else {
    Cow::Owned(format!("_{}", mixed_case_package_name_encode(name)))
  };
  if name.starts_with('@') {
    name.replace('/', "+").into()
  } else {
    name
  }
}

#[cfg(test)]
mod test {
  use deno_npm::NpmPackageCacheFolderId;
  use deno_semver::package::PackageNv;

  use super::*;

  #[test]
  fn test_get_package_folder_id_folder_name() {
    let cases = vec![
      (
        NpmPackageCacheFolderId {
          nv: PackageNv::from_str("@types/foo@1.2.3").unwrap(),
          copy_index: 1,
        },
        "@types+foo@1.2.3_1".to_string(),
      ),
      (
        NpmPackageCacheFolderId {
          nv: PackageNv::from_str("JSON@3.2.1").unwrap(),
          copy_index: 0,
        },
        "_jjju6tq@3.2.1".to_string(),
      ),
    ];
    for (input, output) in cases {
      assert_eq!(get_package_folder_id_folder_name(&input), output);
      let folder_id = get_package_folder_id_from_folder_name(&output).unwrap();
      assert_eq!(folder_id, input);
    }
  }
}
