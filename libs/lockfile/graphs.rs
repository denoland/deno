// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;

use deno_semver::SmallStackString;
use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageKind;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;

use crate::NpmPackageInfo;
use crate::PackagesContent;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum LockfilePkgId {
  Npm(LockfileNpmPackageId),
  Jsr(LockfileJsrPkgNv),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct LockfileJsrPkgNv(PackageNv);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct LockfileNpmPackageId(StackString);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum LockfilePkgReq {
  Jsr(PackageReq),
  Npm(PackageReq),
}

impl LockfilePkgReq {
  pub fn from_jsr_dep(dep: JsrDepPackageReq) -> Self {
    match dep.kind {
      deno_semver::package::PackageKind::Jsr => LockfilePkgReq::Jsr(dep.req),
      deno_semver::package::PackageKind::Npm => LockfilePkgReq::Npm(dep.req),
    }
  }

  pub fn into_jsr_dep(self) -> JsrDepPackageReq {
    match self {
      LockfilePkgReq::Jsr(req) => JsrDepPackageReq::jsr(req),
      LockfilePkgReq::Npm(req) => JsrDepPackageReq::npm(req),
    }
  }

  pub fn req(&self) -> &PackageReq {
    match self {
      LockfilePkgReq::Jsr(req) => req,
      LockfilePkgReq::Npm(req) => req,
    }
  }

  pub fn kind(&self) -> PackageKind {
    match self {
      LockfilePkgReq::Jsr(_) => PackageKind::Jsr,
      LockfilePkgReq::Npm(_) => PackageKind::Npm,
    }
  }
}

#[derive(Debug)]
enum LockfileGraphPackage {
  Jsr(LockfileJsrGraphPackage),
  Npm(LockfileNpmGraphPackage),
}

#[derive(Debug)]
struct LockfileNpmGraphPackage {
  dependents: HashSet<LockfilePkgId>,
  integrity: Option<String>,
  dependencies: BTreeMap<StackString, LockfileNpmPackageId>,
  optional_dependencies: BTreeMap<StackString, LockfileNpmPackageId>,
  optional_peers: BTreeMap<StackString, LockfileNpmPackageId>,
  os: Vec<SmallStackString>,
  cpu: Vec<SmallStackString>,
  tarball: Option<StackString>,
  deprecated: bool,
  scripts: bool,
  bin: bool,
}

impl LockfileNpmGraphPackage {
  pub fn all_dependency_ids(
    &self,
  ) -> impl Iterator<Item = &LockfileNpmPackageId> {
    self
      .dependencies
      .values()
      .chain(self.optional_dependencies.values())
      .chain(self.optional_peers.values())
  }
}

#[derive(Debug)]
struct LockfileJsrGraphPackage {
  dependents: HashSet<LockfilePkgId>,
  integrity: String,
  dependencies: BTreeSet<LockfilePkgReq>,
}

/// Graph used to analyze a lockfile to determine which packages
/// and remotes can be removed based on config file changes.
pub struct LockfilePackageGraph {
  root_packages: HashMap<LockfilePkgReq, LockfilePkgId>,
  packages: HashMap<LockfilePkgId, LockfileGraphPackage>,
  remotes: BTreeMap<String, String>,
}

impl LockfilePackageGraph {
  pub fn from_lockfile(
    content: PackagesContent,
    remotes: BTreeMap<String, String>,
  ) -> Self {
    let mut root_packages =
      HashMap::<LockfilePkgReq, LockfilePkgId>::with_capacity(
        content.specifiers.len(),
      );
    // collect the specifiers to version mappings
    let package_count =
      content.specifiers.len() + content.jsr.len() + content.npm.len();
    let mut packages = HashMap::with_capacity(package_count);
    for (dep, value) in content.specifiers {
      match dep.kind {
        deno_semver::package::PackageKind::Jsr => {
          let Ok(version) = Version::parse_standard(&value) else {
            continue;
          };
          let nv = LockfilePkgId::Jsr(LockfileJsrPkgNv(PackageNv {
            name: dep.req.name.clone(),
            version,
          }));
          root_packages.insert(LockfilePkgReq::Jsr(dep.req), nv);
        }
        deno_semver::package::PackageKind::Npm => {
          let id = LockfileNpmPackageId({
            let mut text =
              StackString::with_capacity(dep.req.name.len() + 1 + value.len());
            text.push_str(&dep.req.name);
            text.push('@');
            text.push_str(&value);
            text
          });
          root_packages
            .insert(LockfilePkgReq::Npm(dep.req), LockfilePkgId::Npm(id));
        }
      }
    }

    for (nv, content_package) in content.jsr {
      packages.insert(
        LockfilePkgId::Jsr(LockfileJsrPkgNv(nv.clone())),
        LockfileGraphPackage::Jsr(LockfileJsrGraphPackage {
          dependents: HashSet::new(),
          integrity: content_package.integrity.clone(),
          dependencies: content_package
            .dependencies
            .into_iter()
            .map(LockfilePkgReq::from_jsr_dep)
            .collect(),
        }),
      );
    }

    for (id, package) in content.npm {
      packages.insert(
        LockfilePkgId::Npm(LockfileNpmPackageId(id.clone())),
        LockfileGraphPackage::Npm(LockfileNpmGraphPackage {
          dependents: HashSet::new(),
          integrity: package.integrity.clone(),
          dependencies: package
            .dependencies
            .iter()
            .map(|(key, dep_id)| {
              (key.clone(), LockfileNpmPackageId(dep_id.clone()))
            })
            .collect(),
          optional_dependencies: package
            .optional_dependencies
            .iter()
            .map(|(name, dep_id)| {
              (name.clone(), LockfileNpmPackageId(dep_id.clone()))
            })
            .collect(),
          cpu: package.cpu.clone(),
          os: package.os.clone(),
          tarball: package.tarball.clone(),
          deprecated: package.deprecated,
          scripts: package.scripts,
          bin: package.bin,
          optional_peers: package
            .optional_peers
            .iter()
            .map(|(name, dep_id)| {
              (name.clone(), LockfileNpmPackageId(dep_id.clone()))
            })
            .collect(),
        }),
      );
    }

    let pkg_ids = packages.keys().cloned().collect::<Vec<_>>();
    for pkg_id in pkg_ids {
      if let Some(pkg) = packages.get(&pkg_id) {
        let dependency_ids = match pkg {
          LockfileGraphPackage::Jsr(pkg) => pkg
            .dependencies
            .iter()
            .filter_map(|req| root_packages.get(req))
            .cloned()
            .collect::<Vec<_>>(),
          LockfileGraphPackage::Npm(pkg) => pkg
            .all_dependency_ids()
            .cloned()
            .map(LockfilePkgId::Npm)
            .collect::<Vec<_>>(),
        };

        for dep_id in dependency_ids {
          if let Some(pkg) = packages.get_mut(&dep_id) {
            match pkg {
              LockfileGraphPackage::Jsr(pkg) => {
                pkg.dependents.insert(pkg_id.clone());
              }
              LockfileGraphPackage::Npm(pkg) => {
                pkg.dependents.insert(pkg_id.clone());
              }
            }
          }
        }
      }
    }

    Self {
      root_packages,
      packages,
      remotes,
    }
  }

  pub fn remove_links(
    &mut self,
    package_reqs: impl Iterator<Item = JsrDepPackageReq>,
  ) {
    let mut pending_ids = Vec::new();
    for pkg_req in package_reqs {
      for (root_pkg, id) in &self.root_packages {
        if pkg_req.kind == root_pkg.kind()
          && pkg_req.req.name == root_pkg.req().name
        {
          pending_ids.push(id.clone());
        }
      }
    }
    for id in &pending_ids {
      self.remove_root_pkg_by_id(id);
    }
  }

  pub fn remove_root_packages(
    &mut self,
    package_reqs: impl Iterator<Item = JsrDepPackageReq>,
  ) {
    let package_reqs = package_reqs.map(LockfilePkgReq::from_jsr_dep);
    for req in package_reqs {
      if let Some(id) = self.root_packages.get(&req).cloned() {
        self.remove_root_pkg_by_id(&id);
      }
    }
  }

  fn remove_root_pkg_by_id(&mut self, id: &LockfilePkgId) {
    // The ideal goal here is to only disassociate the package
    // from the root so that the current dependencies can be
    // reused in the new dependency resolution thereby causing
    // minimal changes in the lockfile. After we let deno_npm or
    // deno_graph clean up any stragglers.
    //
    // We're currently achieving this goal with npm packages, but
    // jsr packages are still lacking deduplication functionality
    // in deno_graph. So, we've taken a halfway step where npm deps
    // disassociate from the root only, but jsr deps and all their
    // jsr connections need to be purged from the lockfile.
    match id {
      LockfilePkgId::Jsr(_) => {
        let mut root_ids_to_remove = Vec::with_capacity(self.packages.len());
        let mut pending_ids = Vec::with_capacity(self.packages.len());
        pending_ids.push(id.clone());
        while let Some(id) = pending_ids.pop() {
          root_ids_to_remove.push(id.clone());
          let Some(pkg) = self.packages.get_mut(&id) else {
            continue;
          };
          match pkg {
            LockfileGraphPackage::Jsr(pkg) => {
              pending_ids.extend(
                pkg
                  .dependencies
                  .iter()
                  .filter_map(|req| self.root_packages.get(req))
                  .cloned(),
              );
              pending_ids.extend(pkg.dependents.drain());
              self.packages.remove(&id);
            }
            LockfileGraphPackage::Npm(_) => {}
          }
        }

        // sort and dedup for binary search
        root_ids_to_remove.sort();
        root_ids_to_remove.dedup();
        self.root_packages.retain(|_, pkg_id| {
          root_ids_to_remove.binary_search(pkg_id).is_err()
        });
      }
      LockfilePkgId::Npm(_) => {
        self.root_packages.retain(|_, pkg_id| pkg_id != id);
      }
    }
  }

  pub fn populate_packages(
    self,
    packages: &mut PackagesContent,
    remotes: &mut BTreeMap<String, String>,
  ) {
    *remotes = self.remotes;

    for (id, package) in self.packages {
      match package {
        LockfileGraphPackage::Jsr(package) => {
          packages.jsr.insert(
            match id {
              LockfilePkgId::Jsr(nv) => nv.0,
              LockfilePkgId::Npm(_) => unreachable!(),
            },
            crate::JsrPackageInfo {
              integrity: package.integrity,
              dependencies: package
                .dependencies
                .into_iter()
                .filter(|dep| self.root_packages.contains_key(dep))
                .map(|req| req.into_jsr_dep())
                .collect(),
            },
          );
        }
        LockfileGraphPackage::Npm(package) => {
          packages.npm.insert(
            match id {
              LockfilePkgId::Jsr(_) => unreachable!(),
              LockfilePkgId::Npm(id) => id.0,
            },
            NpmPackageInfo {
              integrity: package.integrity,
              dependencies: package
                .dependencies
                .into_iter()
                .map(|(name, id)| (name, id.0))
                .collect(),
              cpu: package.cpu,
              os: package.os,
              tarball: package.tarball.clone(),
              optional_dependencies: package
                .optional_dependencies
                .into_iter()
                .map(|(name, id)| (name, id.0))
                .collect(),
              deprecated: package.deprecated,
              scripts: package.scripts,
              bin: package.bin,
              optional_peers: package
                .optional_peers
                .into_iter()
                .map(|(name, id)| (name, id.0))
                .collect(),
            },
          );
        }
      }
    }

    for (req, id) in self.root_packages {
      let value = match &id {
        LockfilePkgId::Jsr(nv) => {
          nv.0.version.to_custom_string::<SmallStackString>()
        }
        LockfilePkgId::Npm(id) => id
          .0
          .as_str()
          .strip_prefix(req.req().name.as_str())
          .unwrap()
          .strip_prefix("@")
          .unwrap()
          .into(),
      };
      packages.specifiers.insert(req.into_jsr_dep(), value);
    }
  }
}
