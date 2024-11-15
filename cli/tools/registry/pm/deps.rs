use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_config::deno_json::ConfigFile;
use deno_config::deno_json::ConfigFileRc;
use deno_config::workspace::Workspace;
use deno_config::workspace::WorkspaceDirectory;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
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
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::package::PackageReqReference;
use deno_semver::Version;
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
  pub fn is_package_json(&self) -> bool {
    matches!(self, DepLocation::PackageJson(..))
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

#[derive(Clone, Debug)]
pub enum DepKind {
  Npm,
  Jsr,
}

impl DepKind {
  fn scheme(&self) -> &'static str {
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

macro_rules! const_assert {
  ($x:expr $(,)?) => {
    #[allow(unknown_lints, eq_op)]
    const _: [();
      0 - !{
        const ASSERT: bool = $x;
        ASSERT
      } as usize] = [];
  };
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
  fn push(&mut self, part: KeyPart) {
    self.parts.push(part)
  }
}

#[derive(Clone, Debug)]
pub struct Dep {
  pub req: PackageReq,
  pub kind: DepKind,
  pub location: DepLocation,
  pub id: DepId,
  // specifier: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedDep {
  dep: Dep,
  resolved_version: Option<Version>,
}

#[derive(Debug)]
pub struct WorkspaceDeps {
  deps: Vec<Dep>,
}

fn import_map_entries<'a>(
  import_map: &'a ImportMap,
) -> impl Iterator<Item = (KeyPath, SpecifierMapEntry<'a>)> {
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
        full_path.push(KeyPart::Imports);
        full_path.push(KeyPart::String(entry.raw_key.to_string()));
        (full_path, entry)
      })
    }))
}

fn deno_json_import_map(
  deno_json: &ConfigFile,
) -> Result<Option<(ImportMapWithDiagnostics, ImportMapKind)>, AnyError> {
  let Some((url, value)) = deno_json.to_import_map_value(|path| {
    std::fs::read_to_string(path).map_err(Into::into)
  })?
  else {
    return Ok(None);
  };

  import_map::parse_from_value(deno_json.specifier.clone(), value)
    .map_err(Into::into)
    .map(|import_map| {
      let kind = if &*url == &deno_json.specifier {
        ImportMapKind::Inline
      } else {
        ImportMapKind::Outline
      };
      Some((import_map, kind))
    })
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

fn add_deps_from_deno_json(deno_json: &Arc<ConfigFile>, deps: &mut Vec<Dep>) {
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
    let req = match JsrDepPackageReq::from_str(value.as_str()) {
      Ok(req) => req,
      Err(err) => {
        log::warn!("failed to parse package req \"{}\": {err}", value.as_str());
        continue;
      }
    };
    let id = DepId(deps.len());
    deps.push(Dep {
      location: DepLocation::DenoJson(
        deno_json.clone(),
        key_path,
        import_map_kind,
      ),
      kind,
      req: req.req,
      id,
    });
  }
}

fn add_deps_from_package_json(
  package_json: &PackageJsonRc,
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
      deno_package_json::PackageJsonDepValue::Req(package_req) => {
        let id = DepId(deps.len());
        deps.push(Dep {
          id,
          kind: DepKind::Npm,
          location: DepLocation::PackageJson(
            package_json.clone(),
            KeyPath::from_parts([package_dep_kind.into(), k.into()]),
          ),
          req: package_req,
        })
      }
      deno_package_json::PackageJsonDepValue::Workspace(_) => continue,
    }
  }
}

fn deps_from_workspace(
  workspace: &Arc<Workspace>,
) -> Result<Vec<Dep>, AnyError> {
  let mut deps = Vec::with_capacity(32);
  for deno_json in workspace.deno_jsons() {
    eprintln!("deno_json: {}", deno_json.specifier);
    add_deps_from_deno_json(deno_json, &mut deps);
  }
  for package_json in workspace.package_jsons() {
    eprintln!("package_json: {}", package_json.path.display());
    add_deps_from_package_json(package_json, &mut deps);
  }

  Ok(deps)
}

#[derive(Default, Clone)]
enum ResolveState {
  #[default]
  NotYet,
  Unresolved,
  Resolved(PackageNv),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DepId(usize);

#[derive(Debug, Clone)]
pub enum Change {
  Update(DepId, VersionReq),
}

pub struct DepManager {
  deps: Vec<Dep>,
  resolved_versions: Vec<Option<PackageNv>>,
  latest_versions: Vec<Option<PackageNv>>,

  pending_changes: Vec<Change>,

  loaded_roots: bool,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  jsr_fetch_resolver: Arc<JsrFetchResolver>,
  npm_fetch_resolver: Arc<NpmFetchResolver>,
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
      loaded_roots: false,
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
    args: DepManagerArgs,
  ) -> Result<Self, AnyError> {
    let mut deps = Vec::with_capacity(32);
    if let Some(deno_json) = workspace_dir.maybe_deno_json() {
      add_deps_from_deno_json(deno_json, &mut deps);
    }
    if let Some(package_json) = workspace_dir.maybe_pkg_json() {
      add_deps_from_package_json(package_json, &mut deps);
    }

    Ok(Self::with_deps_args(deps, args))
  }
  pub fn from_workspace(
    workspace: &Arc<Workspace>,
    args: DepManagerArgs,
  ) -> Result<Self, AnyError> {
    let deps = deps_from_workspace(workspace)?;
    Ok(Self::with_deps_args(deps, args))
  }

  async fn load_roots(&mut self) -> Result<(), AnyError> {
    if self.loaded_roots {
      return Ok(());
    }

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

  pub async fn resolve_versions(&mut self) -> Result<(), AnyError> {
    self.load_roots().await?;

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

  pub fn resolved_version(
    &self,
    dep_id: DepId,
  ) -> Result<Option<&PackageNv>, AnyError> {
    if self.resolved_versions.len() < self.deps.len() {
      return Err(deno_core::anyhow::anyhow!(
        "Versions haven't been resolved yet"
      ));
    }

    Ok(self.resolved_versions[dep_id.0].as_ref())
  }

  pub fn resolved_versions(&self) -> &[Option<PackageNv>] {
    &self.resolved_versions
  }

  pub async fn fetch_latest_versions(
    &mut self,
    semver_compatible: bool,
  ) -> Result<(), AnyError> {
    let latest_tag_req = deno_semver::VersionReq::from_raw_text_and_inner(
      "latest".into(),
      deno_semver::RangeSetOrTag::Tag("latest".into()),
    );
    let mut latest = Vec::with_capacity(self.deps.len());

    let sema = Semaphore::new(32);
    let mut futs = FuturesOrdered::new();

    for dep in &self.deps {
      match dep.kind {
        DepKind::Npm => futs.push_back(
          async {
            let req = if semver_compatible {
              Cow::Borrowed(&dep.req)
            } else {
              Cow::Owned(PackageReq {
                name: dep.req.name.clone(),
                version_req: latest_tag_req.clone(),
              })
            };
            let _permit = sema.acquire().await;
            self.npm_fetch_resolver.req_to_nv(&req).await
          }
          .boxed_local(),
        ),
        DepKind::Jsr => futs.push_back(
          async {
            let req = if semver_compatible {
              Cow::Borrowed(&dep.req)
            } else {
              Cow::Owned(PackageReq {
                name: dep.req.name.clone(),
                version_req: deno_semver::WILDCARD_VERSION_REQ.clone(),
              })
            };
            let _permit = sema.acquire().await;
            self.jsr_fetch_resolver.req_to_nv(&req).await
          }
          .boxed_local(),
        ),
      }
    }
    while let Some(nv) = futs.next().await {
      latest.push(nv);
    }
    self.latest_versions = latest;

    Ok(())
  }

  pub fn latest_versions(&self) -> &[Option<PackageNv>] {
    &self.latest_versions
  }

  pub fn deps_with_latest_versions(
    &self,
  ) -> impl IntoIterator<Item = (DepId, Option<PackageNv>)> + '_ {
    self
      .latest_versions
      .iter()
      .enumerate()
      .map(|(i, latest)| (DepId(i), latest.clone()))
  }

  pub fn deps(&self) -> &[Dep] {
    &self.deps
  }

  pub fn update_dep(&mut self, dep_id: DepId, new_version_req: VersionReq) {
    self
      .pending_changes
      .push(Change::Update(dep_id, new_version_req))
  }

  pub async fn commit_changes(&mut self) -> Result<(), AnyError> {
    let changes = std::mem::take(&mut self.pending_changes);
    // let mut config_updaters = HashMap::new();
    for change in changes {
      match change {
        Change::Update(dep_id, version_req) => {
          let dep = &self.deps[dep_id.0];
          match &dep.location {
            DepLocation::DenoJson(arc, key_path, import_map_kind) => {
              if matches!(import_map_kind, ImportMapKind::Outline) {
                // not supported
                continue;
              }
              let mut updater = ConfigUpdater::new(
                super::ConfigKind::DenoJson,
                // TODO: unwrap is sus
                arc.specifier.to_file_path().unwrap(),
              )?;
              let Some(property) = updater.get_property_for_mutation(&key_path)
              else {
                log::warn!(
                  "failed to find property at path {key_path:?} for file {}",
                  arc.specifier
                );
                continue;
              };
              let value = property.value().unwrap();
              let Some(string) = value.as_string_lit() else {
                log::warn!("malformed entry");
                continue;
              };
              let Ok(string_value) = string.decoded_value() else {
                log::warn!("malformed string: {string:?}");
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
              let new_value =
                format!("{}:{}", dep.kind.scheme(), req_reference);
              property
                .set_value(jsonc_parser::cst::CstInputValue::String(new_value));
              updater.commit()?;
            }
            DepLocation::PackageJson(arc, key_path) => {
              eprintln!("here: {dep:?}");
              let mut updater = ConfigUpdater::new(
                super::ConfigKind::PackageJson,
                arc.path.clone(),
              )?;
              let Some(property) = updater.get_property_for_mutation(&key_path)
              else {
                log::warn!(
                  "failed to find property at path {key_path:?} for file {}",
                  arc.path.display()
                );
                continue;
              };
              let value = property.value().unwrap();
              let Some(string) = value.as_string_lit() else {
                log::warn!("malformed entry");
                continue;
              };
              let Ok(string_value) = string.decoded_value() else {
                log::warn!("malformed string: {string:?}");
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
              updater.commit()?;
            }
          }
        }
      }
    }

    Ok(())
  }
}
