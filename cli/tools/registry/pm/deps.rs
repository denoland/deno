// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_config::deno_json::ConfigFile;
use deno_config::deno_json::ConfigFileRc;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceDirectory;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures::future::try_join;
use deno_core::futures::stream::FuturesOrdered;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_graph::FillFromLockfileOptions;
use deno_package_json::PackageJsonDepValue;
use deno_package_json::PackageJsonDepValueParseError;
use deno_package_json::PackageJsonRc;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::package::PackageReqReference;
use deno_semver::VersionReq;
use import_map::ImportMap;
use import_map::ImportMapWithDiagnostics;
use import_map::SpecifierMapEntry;
use indexmap::IndexMap;
use tokio::sync::Semaphore;

use crate::args::CliLockfile;
use crate::graph_container::MainModuleGraphContainer;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::jsr::JsrFetchResolver;
use crate::module_loader::ModuleLoadPreparer;
use crate::npm::CliNpmResolver;
use crate::npm::NpmFetchResolver;

use super::ConfigUpdater;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportMapKind {
  Inline,
  Outline,
}

#[derive(Clone)]
pub enum DepLocation {
  DenoJson(ConfigFileRc, KeyPath, ImportMapKind),
  PackageJson(PackageJsonRc, KeyPath),
}

impl DepLocation {
  pub fn is_deno_json(&self) -> bool {
    matches!(self, DepLocation::DenoJson(..))
  }

  pub fn file_path(&self) -> Cow<std::path::Path> {
    match self {
      DepLocation::DenoJson(arc, _, _) => {
        Cow::Owned(arc.specifier.to_file_path().unwrap())
      }
      DepLocation::PackageJson(arc, _) => Cow::Borrowed(arc.path.as_ref()),
    }
  }
  fn config_kind(&self) -> super::ConfigKind {
    match self {
      DepLocation::DenoJson(_, _, _) => super::ConfigKind::DenoJson,
      DepLocation::PackageJson(_, _) => super::ConfigKind::PackageJson,
    }
  }
}

struct DebugAdapter<T>(T);

impl<'a> std::fmt::Debug for DebugAdapter<&'a ConfigFileRc> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ConfigFile")
      .field("specifier", &self.0.specifier)
      .finish()
  }
}
impl<'a> std::fmt::Debug for DebugAdapter<&'a PackageJsonRc> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("PackageJson")
      .field("path", &self.0.path)
      .finish()
  }
}

impl std::fmt::Debug for DepLocation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      DepLocation::DenoJson(arc, key_path, kind) => {
        let mut debug = f.debug_tuple("DenoJson");
        debug
          .field(&DebugAdapter(arc))
          .field(key_path)
          .field(kind)
          .finish()
      }
      DepLocation::PackageJson(arc, key_path) => {
        let mut debug = f.debug_tuple("PackageJson");
        debug.field(&DebugAdapter(arc)).field(key_path).finish()
      }
    }
  }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DepKind {
  Npm,
  Jsr,
}

impl DepKind {
  pub fn scheme(&self) -> &'static str {
    match self {
      DepKind::Npm => "npm",
      DepKind::Jsr => "jsr",
    }
  }
}

#[derive(Clone, Debug)]
pub enum KeyPart {
  Imports,
  Scopes,
  Dependencies,
  DevDependencies,
  String(String),
}

impl From<String> for KeyPart {
  fn from(value: String) -> Self {
    KeyPart::String(value)
  }
}

impl From<PackageJsonDepKind> for KeyPart {
  fn from(value: PackageJsonDepKind) -> Self {
    match value {
      PackageJsonDepKind::Normal => Self::Dependencies,
      PackageJsonDepKind::Dev => Self::DevDependencies,
    }
  }
}

impl KeyPart {
  pub fn as_str(&self) -> &str {
    match self {
      KeyPart::Imports => "imports",
      KeyPart::Scopes => "scopes",
      KeyPart::Dependencies => "dependencies",
      KeyPart::DevDependencies => "devDependencies",
      KeyPart::String(s) => s,
    }
  }
}

#[derive(Clone, Debug)]
pub struct KeyPath {
  pub parts: Vec<KeyPart>,
}

impl KeyPath {
  fn from_parts(parts: impl IntoIterator<Item = KeyPart>) -> Self {
    Self {
      parts: parts.into_iter().collect(),
    }
  }
  fn last(&self) -> Option<&KeyPart> {
    self.parts.last()
  }
  fn push(&mut self, part: KeyPart) {
    self.parts.push(part)
  }
}

#[derive(Clone, Debug)]
pub struct Dep {
  pub req: PackageReq,
  pub kind: DepKind,
  pub location: DepLocation,
  #[allow(dead_code)]
  pub id: DepId,
  #[allow(dead_code)]
  pub alias: Option<String>,
}

impl Dep {
  pub fn prefixed_req(&self) -> String {
    format!("{}:{}", self.kind.scheme(), self.req)
  }
}

fn import_map_entries(
  import_map: &ImportMap,
) -> impl Iterator<Item = (KeyPath, SpecifierMapEntry<'_>)> {
  import_map
    .imports()
    .entries()
    .map(|entry| {
      (
        KeyPath::from_parts([
          KeyPart::Imports,
          KeyPart::String(entry.raw_key.into()),
        ]),
        entry,
      )
    })
    .chain(import_map.scopes().flat_map(|scope| {
      let path = KeyPath::from_parts([
        KeyPart::Scopes,
        scope.raw_key.to_string().into(),
      ]);

      scope.imports.entries().map(move |entry| {
        let mut full_path = path.clone();
        full_path.push(KeyPart::String(entry.raw_key.to_string()));
        (full_path, entry)
      })
    }))
}

fn to_import_map_value_from_imports(
  deno_json: &ConfigFile,
) -> serde_json::Value {
  let mut value = serde_json::Map::with_capacity(2);
  if let Some(imports) = &deno_json.json.imports {
    value.insert("imports".to_string(), imports.clone());
  }
  if let Some(scopes) = &deno_json.json.scopes {
    value.insert("scopes".to_string(), scopes.clone());
  }
  serde_json::Value::Object(value)
}

fn deno_json_import_map(
  deno_json: &ConfigFile,
) -> Result<Option<(ImportMapWithDiagnostics, ImportMapKind)>, AnyError> {
  let (value, kind) =
    if deno_json.json.imports.is_some() || deno_json.json.scopes.is_some() {
      (
        to_import_map_value_from_imports(deno_json),
        ImportMapKind::Inline,
      )
    } else {
      match deno_json.to_import_map_path()? {
        Some(path) => {
          let text = std::fs::read_to_string(&path)?;
          let value = serde_json::from_str(&text)?;
          (value, ImportMapKind::Outline)
        }
        None => return Ok(None),
      }
    };

  import_map::parse_from_value(deno_json.specifier.clone(), value)
    .map_err(Into::into)
    .map(|import_map| Some((import_map, kind)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageJsonDepKind {
  Normal,
  Dev,
}

type PackageJsonDeps = IndexMap<
  String,
  Result<
    (PackageJsonDepKind, PackageJsonDepValue),
    PackageJsonDepValueParseError,
  >,
>;

/// Resolve the package.json's dependencies.
// TODO(nathanwhit): Remove once we update deno_package_json with dev deps split out
fn resolve_local_package_json_deps(
  package_json: &PackageJsonRc,
) -> PackageJsonDeps {
  /// Gets the name and raw version constraint for a registry info or
  /// package.json dependency entry taking into account npm package aliases.
  fn parse_dep_entry_name_and_raw_version<'a>(
    key: &'a str,
    value: &'a str,
  ) -> (&'a str, &'a str) {
    if let Some(package_and_version) = value.strip_prefix("npm:") {
      if let Some((name, version)) = package_and_version.rsplit_once('@') {
        // if empty, then the name was scoped and there's no version
        if name.is_empty() {
          (package_and_version, "*")
        } else {
          (name, version)
        }
      } else {
        (package_and_version, "*")
      }
    } else {
      (key, value)
    }
  }

  fn parse_entry(
    key: &str,
    value: &str,
  ) -> Result<PackageJsonDepValue, PackageJsonDepValueParseError> {
    if let Some(workspace_key) = value.strip_prefix("workspace:") {
      let version_req = VersionReq::parse_from_npm(workspace_key)?;
      return Ok(PackageJsonDepValue::Workspace(version_req));
    }
    if value.starts_with("file:")
      || value.starts_with("git:")
      || value.starts_with("http:")
      || value.starts_with("https:")
    {
      return Err(PackageJsonDepValueParseError::Unsupported {
        scheme: value.split(':').next().unwrap().to_string(),
      });
    }
    let (name, version_req) = parse_dep_entry_name_and_raw_version(key, value);
    let result = VersionReq::parse_from_npm(version_req);
    match result {
      Ok(version_req) => Ok(PackageJsonDepValue::Req(PackageReq {
        name: name.to_string(),
        version_req,
      })),
      Err(err) => Err(PackageJsonDepValueParseError::VersionReq(err)),
    }
  }

  fn insert_deps(
    deps: Option<&IndexMap<String, String>>,
    result: &mut PackageJsonDeps,
    kind: PackageJsonDepKind,
  ) {
    if let Some(deps) = deps {
      for (key, value) in deps {
        result.entry(key.to_string()).or_insert_with(|| {
          parse_entry(key, value).map(|entry| (kind, entry))
        });
      }
    }
  }

  let deps = package_json.dependencies.as_ref();
  let dev_deps = package_json.dev_dependencies.as_ref();
  let mut result = IndexMap::new();

  // favors the deps over dev_deps
  insert_deps(deps, &mut result, PackageJsonDepKind::Normal);
  insert_deps(dev_deps, &mut result, PackageJsonDepKind::Dev);

  result
}

fn add_deps_from_deno_json(
  deno_json: &Arc<ConfigFile>,
  mut filter: impl DepFilter,
  deps: &mut Vec<Dep>,
) {
  let (import_map, import_map_kind) = match deno_json_import_map(deno_json) {
    Ok(Some((import_map, import_map_kind))) => (import_map, import_map_kind),
    Ok(None) => return,
    Err(e) => {
      log::warn!("failed to parse imports from {}: {e}", &deno_json.specifier);
      return;
    }
  };
  for (key_path, entry) in import_map_entries(&import_map.import_map) {
    let Some(value) = entry.value else { continue };
    let kind = match value.scheme() {
      "npm" => DepKind::Npm,
      "jsr" => DepKind::Jsr,
      _ => continue,
    };
    let req = match parse_req_reference(value.as_str(), kind) {
      Ok(req) => req.req.clone(),
      Err(err) => {
        log::warn!("failed to parse package req \"{}\": {err}", value.as_str());
        continue;
      }
    };
    let alias: &str = key_path.last().unwrap().as_str().trim_end_matches('/');
    let alias = (alias != req.name).then(|| alias.to_string());
    if !filter.should_include(alias.as_deref(), &req, kind) {
      continue;
    }
    let id = DepId(deps.len());
    deps.push(Dep {
      location: DepLocation::DenoJson(
        deno_json.clone(),
        key_path,
        import_map_kind,
      ),
      kind,
      req,
      id,
      alias,
    });
  }
}

fn add_deps_from_package_json(
  package_json: &PackageJsonRc,
  mut filter: impl DepFilter,
  deps: &mut Vec<Dep>,
) {
  let package_json_deps = resolve_local_package_json_deps(package_json);
  for (k, v) in package_json_deps {
    let (package_dep_kind, v) = match v {
      Ok((k, v)) => (k, v),
      Err(e) => {
        log::warn!("bad package json dep value: {e}");
        continue;
      }
    };
    match v {
      deno_package_json::PackageJsonDepValue::Req(req) => {
        let alias = k.as_str();
        let alias = (alias != req.name).then(|| alias.to_string());
        if !filter.should_include(alias.as_deref(), &req, DepKind::Npm) {
          continue;
        }
        let id = DepId(deps.len());
        deps.push(Dep {
          id,
          kind: DepKind::Npm,
          location: DepLocation::PackageJson(
            package_json.clone(),
            KeyPath::from_parts([package_dep_kind.into(), k.into()]),
          ),
          req,
          alias,
        })
      }
      deno_package_json::PackageJsonDepValue::Workspace(_) => continue,
    }
  }
}

fn deps_from_workspace(
  workspace: &Arc<Workspace>,
  dep_filter: impl DepFilter,
) -> Result<Vec<Dep>, AnyError> {
  let mut deps = Vec::with_capacity(32);
  for deno_json in workspace.deno_jsons() {
    add_deps_from_deno_json(deno_json, dep_filter, &mut deps);
  }
  for package_json in workspace.package_jsons() {
    add_deps_from_package_json(package_json, dep_filter, &mut deps);
  }

  Ok(deps)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DepId(usize);

#[derive(Debug, Clone)]
pub enum Change {
  Update(DepId, VersionReq),
}

pub trait DepFilter: Copy {
  fn should_include(
    &mut self,
    alias: Option<&str>,
    package_req: &PackageReq,
    dep_kind: DepKind,
  ) -> bool;
}

impl<T> DepFilter for T
where
  T: FnMut(Option<&str>, &PackageReq, DepKind) -> bool + Copy,
{
  fn should_include<'a>(
    &mut self,
    alias: Option<&'a str>,
    package_req: &'a PackageReq,
    dep_kind: DepKind,
  ) -> bool {
    (*self)(alias, package_req, dep_kind)
  }
}

#[derive(Clone, Debug)]
pub struct PackageLatestVersion {
  pub semver_compatible: Option<PackageNv>,
  pub latest: Option<PackageNv>,
}

pub struct DepManager {
  deps: Vec<Dep>,
  resolved_versions: Vec<Option<PackageNv>>,
  latest_versions: Vec<PackageLatestVersion>,

  pending_changes: Vec<Change>,

  dependencies_resolved: AtomicBool,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  // TODO(nathanwhit): probably shouldn't be pub
  pub(crate) jsr_fetch_resolver: Arc<JsrFetchResolver>,
  pub(crate) npm_fetch_resolver: Arc<NpmFetchResolver>,
  npm_resolver: Arc<dyn CliNpmResolver>,
  permissions_container: PermissionsContainer,
  main_module_graph_container: Arc<MainModuleGraphContainer>,
  lockfile: Option<Arc<CliLockfile>>,
}

pub struct DepManagerArgs {
  pub module_load_preparer: Arc<ModuleLoadPreparer>,
  pub jsr_fetch_resolver: Arc<JsrFetchResolver>,
  pub npm_fetch_resolver: Arc<NpmFetchResolver>,
  pub npm_resolver: Arc<dyn CliNpmResolver>,
  pub permissions_container: PermissionsContainer,
  pub main_module_graph_container: Arc<MainModuleGraphContainer>,
  pub lockfile: Option<Arc<CliLockfile>>,
}

impl DepManager {
  pub fn reloaded_after_modification(self, args: DepManagerArgs) -> Self {
    let mut new = Self::with_deps_args(self.deps, args);
    new.latest_versions = self.latest_versions;
    new
  }
  fn with_deps_args(deps: Vec<Dep>, args: DepManagerArgs) -> Self {
    let DepManagerArgs {
      module_load_preparer,
      jsr_fetch_resolver,
      npm_fetch_resolver,
      npm_resolver,
      permissions_container,
      main_module_graph_container,
      lockfile,
    } = args;
    Self {
      deps,
      resolved_versions: Vec::new(),
      latest_versions: Vec::new(),
      jsr_fetch_resolver,
      dependencies_resolved: AtomicBool::new(false),
      module_load_preparer,
      npm_fetch_resolver,
      npm_resolver,
      permissions_container,
      main_module_graph_container,
      lockfile,
      pending_changes: Vec::new(),
    }
  }
  pub fn from_workspace_dir(
    workspace_dir: &Arc<WorkspaceDirectory>,
    dep_filter: impl DepFilter,
    args: DepManagerArgs,
  ) -> Result<Self, AnyError> {
    let mut deps = Vec::with_capacity(32);
    if let Some(deno_json) = workspace_dir.maybe_deno_json() {
      add_deps_from_deno_json(deno_json, dep_filter, &mut deps);
    }
    if let Some(package_json) = workspace_dir.maybe_pkg_json() {
      add_deps_from_package_json(package_json, dep_filter, &mut deps);
    }

    Ok(Self::with_deps_args(deps, args))
  }
  pub fn from_workspace(
    workspace: &Arc<Workspace>,
    dep_filter: impl DepFilter,
    args: DepManagerArgs,
  ) -> Result<Self, AnyError> {
    let deps = deps_from_workspace(workspace, dep_filter)?;
    Ok(Self::with_deps_args(deps, args))
  }

  async fn run_dependency_resolution(&self) -> Result<(), AnyError> {
    if self
      .dependencies_resolved
      .load(std::sync::atomic::Ordering::Relaxed)
    {
      return Ok(());
    }

    let mut graph_permit = self
      .main_module_graph_container
      .acquire_update_permit()
      .await;
    let graph = graph_permit.graph_mut();
    // populate the information from the lockfile
    if let Some(lockfile) = &self.lockfile {
      let lockfile = lockfile.lock();
      graph.fill_from_lockfile(FillFromLockfileOptions {
        redirects: lockfile
          .content
          .redirects
          .iter()
          .map(|(from, to)| (from.as_str(), to.as_str())),
        package_specifiers: lockfile
          .content
          .packages
          .specifiers
          .iter()
          .map(|(dep, id)| (dep, id.as_str())),
      });
    }

    let npm_resolver = self.npm_resolver.as_managed().unwrap();
    if self.deps.iter().all(|dep| match dep.kind {
      DepKind::Npm => {
        npm_resolver.resolve_pkg_id_from_pkg_req(&dep.req).is_ok()
      }
      DepKind::Jsr => graph.packages.mappings().contains_key(&dep.req),
    }) {
      self
        .dependencies_resolved
        .store(true, std::sync::atomic::Ordering::Relaxed);
      graph_permit.commit();
      return Ok(());
    }

    npm_resolver.ensure_top_level_package_json_install().await?;
    let mut roots = Vec::new();
    let mut info_futures = FuturesUnordered::new();
    for dep in &self.deps {
      if dep.location.is_deno_json() {
        match dep.kind {
          DepKind::Npm => roots.push(
            ModuleSpecifier::parse(&format!("npm:/{}/", dep.req)).unwrap(),
          ),
          DepKind::Jsr => info_futures.push(async {
            if let Some(nv) = self.jsr_fetch_resolver.req_to_nv(&dep.req).await
            {
              if let Some(info) =
                self.jsr_fetch_resolver.package_version_info(&nv).await
              {
                let specifier =
                  ModuleSpecifier::parse(&format!("jsr:/{}/", dep.req))
                    .unwrap();
                return Some((specifier, info));
              }
            }
            None
          }),
        }
      }
    }

    while let Some(info_future) = info_futures.next().await {
      if let Some((specifier, info)) = info_future {
        let exports = info.exports();
        for (k, _) in exports {
          if let Ok(spec) = specifier.join(k) {
            roots.push(spec);
          }
        }
      }
    }

    self
      .module_load_preparer
      .prepare_module_load(
        graph,
        &roots,
        false,
        deno_config::deno_json::TsTypeLib::DenoWindow,
        self.permissions_container.clone(),
        None,
      )
      .await?;

    graph_permit.commit();

    Ok(())
  }

  pub fn resolved_version(&self, id: DepId) -> Option<&PackageNv> {
    self.resolved_versions[id.0].as_ref()
  }

  pub async fn resolve_current_versions(&mut self) -> Result<(), AnyError> {
    self.run_dependency_resolution().await?;

    let graph = self.main_module_graph_container.graph();

    let mut resolved = Vec::with_capacity(self.deps.len());
    let snapshot = self.npm_resolver.as_managed().unwrap().snapshot();
    let resolved_npm = snapshot.package_reqs();
    let resolved_jsr = graph.packages.mappings();
    for dep in &self.deps {
      match dep.kind {
        DepKind::Npm => {
          let resolved_version = resolved_npm.get(&dep.req).cloned();
          resolved.push(resolved_version);
        }
        DepKind::Jsr => {
          let resolved_version = resolved_jsr.get(&dep.req).cloned();
          resolved.push(resolved_version)
        }
      }
    }

    self.resolved_versions = resolved;

    Ok(())
  }

  async fn load_latest_versions(
    &self,
  ) -> Result<Vec<PackageLatestVersion>, AnyError> {
    if self.latest_versions.len() == self.deps.len() {
      return Ok(self.latest_versions.clone());
    }
    let latest_tag_req = deno_semver::VersionReq::from_raw_text_and_inner(
      "latest".into(),
      deno_semver::RangeSetOrTag::Tag("latest".into()),
    );
    let mut latest_versions = Vec::with_capacity(self.deps.len());

    let sema = Semaphore::new(32);
    let mut futs = FuturesOrdered::new();

    for dep in &self.deps {
      match dep.kind {
        DepKind::Npm => futs.push_back(
          async {
            let semver_req = &dep.req;
            let latest_req = PackageReq {
              name: dep.req.name.clone(),
              version_req: latest_tag_req.clone(),
            };
            let _permit = sema.acquire().await;
            let semver_compatible =
              self.npm_fetch_resolver.req_to_nv(semver_req).await;
            let latest = self.npm_fetch_resolver.req_to_nv(&latest_req).await;
            PackageLatestVersion {
              latest,
              semver_compatible,
            }
          }
          .boxed_local(),
        ),
        DepKind::Jsr => futs.push_back(
          async {
            let semver_req = &dep.req;
            let latest_req = PackageReq {
              name: dep.req.name.clone(),
              version_req: deno_semver::WILDCARD_VERSION_REQ.clone(),
            };
            let _permit = sema.acquire().await;
            let semver_compatible =
              self.jsr_fetch_resolver.req_to_nv(semver_req).await;
            let latest = self.jsr_fetch_resolver.req_to_nv(&latest_req).await;
            PackageLatestVersion {
              latest,
              semver_compatible,
            }
          }
          .boxed_local(),
        ),
      }
    }
    while let Some(nv) = futs.next().await {
      latest_versions.push(nv);
    }

    Ok(latest_versions)
  }

  pub async fn resolve_versions(&mut self) -> Result<(), AnyError> {
    let (_, latest_versions) = try_join(
      self.run_dependency_resolution(),
      self.load_latest_versions(),
    )
    .await?;

    self.latest_versions = latest_versions;

    self.resolve_current_versions().await?;

    Ok(())
  }

  pub fn deps_with_resolved_latest_versions(
    &self,
  ) -> impl IntoIterator<Item = (DepId, Option<PackageNv>, PackageLatestVersion)> + '_
  {
    self
      .resolved_versions
      .iter()
      .zip(self.latest_versions.iter())
      .enumerate()
      .map(|(i, (resolved, latest))| {
        (DepId(i), resolved.clone(), latest.clone())
      })
  }

  pub fn get_dep(&self, id: DepId) -> &Dep {
    &self.deps[id.0]
  }

  pub fn update_dep(&mut self, dep_id: DepId, new_version_req: VersionReq) {
    self
      .pending_changes
      .push(Change::Update(dep_id, new_version_req));
  }

  pub fn commit_changes(&mut self) -> Result<(), AnyError> {
    let changes = std::mem::take(&mut self.pending_changes);
    let mut config_updaters = HashMap::new();
    for change in changes {
      match change {
        Change::Update(dep_id, version_req) => {
          // TODO: move most of this to ConfigUpdater
          let dep = &mut self.deps[dep_id.0];
          dep.req.version_req = version_req.clone();
          match &dep.location {
            DepLocation::DenoJson(arc, key_path, import_map_kind) => {
              if matches!(import_map_kind, ImportMapKind::Outline) {
                // not supported
                continue;
              }
              let updater =
                get_or_create_updater(&mut config_updaters, &dep.location)?;

              let Some(property) = updater.get_property_for_mutation(key_path)
              else {
                log::warn!(
                  "failed to find property at path {key_path:?} for file {}",
                  arc.specifier
                );
                continue;
              };
              let Some(string_value) = cst_string_literal(&property) else {
                continue;
              };
              let mut req_reference = match dep.kind {
                DepKind::Npm => NpmPackageReqReference::from_str(&string_value)
                  .unwrap()
                  .into_inner(),
                DepKind::Jsr => JsrPackageReqReference::from_str(&string_value)
                  .unwrap()
                  .into_inner(),
              };
              req_reference.req.version_req = version_req;
              let mut new_value =
                format!("{}:{}", dep.kind.scheme(), req_reference);
              if string_value.ends_with('/') && !new_value.ends_with('/') {
                // the display impl for PackageReqReference maps `/` to the root
                // subpath, but for the import map the trailing `/` is significant
                new_value.push('/');
              }
              if string_value
                .trim_start_matches(format!("{}:", dep.kind.scheme()).as_str())
                .starts_with('/')
              {
                // this is gross
                new_value = new_value.replace(':', ":/");
              }
              property
                .set_value(jsonc_parser::cst::CstInputValue::String(new_value));
            }
            DepLocation::PackageJson(arc, key_path) => {
              let updater =
                get_or_create_updater(&mut config_updaters, &dep.location)?;
              let Some(property) = updater.get_property_for_mutation(key_path)
              else {
                log::warn!(
                  "failed to find property at path {key_path:?} for file {}",
                  arc.path.display()
                );
                continue;
              };
              let Some(string_value) = cst_string_literal(&property) else {
                continue;
              };
              let new_value = if string_value.starts_with("npm:") {
                // aliased
                let rest = string_value.trim_start_matches("npm:");
                let mut parts = rest.split('@');
                let first = parts.next().unwrap();
                if first.is_empty() {
                  let scope_and_name = parts.next().unwrap();
                  format!("npm:@{scope_and_name}@{version_req}")
                } else {
                  format!("npm:{first}@{version_req}")
                }
              } else if string_value.contains(":") {
                bail!("Unexpected package json dependency string: \"{string_value}\" in {}", arc.path.display());
              } else {
                version_req.to_string()
              };
              property
                .set_value(jsonc_parser::cst::CstInputValue::String(new_value));
            }
          }
        }
      }
    }

    for (_, updater) in config_updaters {
      updater.commit()?;
    }

    Ok(())
  }
}

fn get_or_create_updater<'a>(
  config_updaters: &'a mut HashMap<std::path::PathBuf, ConfigUpdater>,
  location: &DepLocation,
) -> Result<&'a mut ConfigUpdater, AnyError> {
  match config_updaters.entry(location.file_path().into_owned()) {
    std::collections::hash_map::Entry::Occupied(occupied_entry) => {
      Ok(occupied_entry.into_mut())
    }
    std::collections::hash_map::Entry::Vacant(vacant_entry) => {
      let updater = ConfigUpdater::new(
        location.config_kind(),
        location.file_path().into_owned(),
      )?;
      Ok(vacant_entry.insert(updater))
    }
  }
}

fn cst_string_literal(
  property: &jsonc_parser::cst::CstObjectProp,
) -> Option<String> {
  // TODO(nathanwhit): ensure this unwrap is safe
  let value = property.value().unwrap();
  let Some(string) = value.as_string_lit() else {
    log::warn!("malformed entry");
    return None;
  };
  let Ok(string_value) = string.decoded_value() else {
    log::warn!("malformed string: {string:?}");
    return None;
  };
  Some(string_value)
}

fn parse_req_reference(
  input: &str,
  kind: DepKind,
) -> Result<
  PackageReqReference,
  deno_semver::package::PackageReqReferenceParseError,
> {
  Ok(match kind {
    DepKind::Npm => NpmPackageReqReference::from_str(input)?.into_inner(),
    DepKind::Jsr => JsrPackageReqReference::from_str(input)?.into_inner(),
  })
}
