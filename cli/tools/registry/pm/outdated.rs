use std::sync::Arc;

use deno_config::deno_json::ConfigFile;
use deno_config::deno_json::ConfigFileRc;
use deno_config::workspace::Workspace;
use deno_core::error::AnyError;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_package_json::PackageJsonDepValue;
use deno_package_json::PackageJsonDepValueParseError;
use deno_package_json::PackageJsonRc;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageReq;
use deno_semver::Version;
use deno_semver::VersionReq;
use import_map::ImportMap;
use import_map::ImportMapWithDiagnostics;
use import_map::SpecifierMapEntry;
use indexmap::IndexMap;
use tokio::sync::Semaphore;

use crate::args::CacheSetting;
use crate::args::Flags;
use crate::args::NpmInstallDepsProvider;
use crate::args::OutdatedFlags;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::jsr::JsrFetchResolver;
use crate::npm::NpmFetchResolver;

#[derive(Clone)]
enum DepLocation {
  DenoJson(ConfigFileRc, KeyPath),
  PackageJson(PackageJsonRc, KeyPath),
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
      DepLocation::DenoJson(arc, key_path) => {
        let mut debug = f.debug_tuple("DenoJson");
        debug.field(&DebugAdapter(arc)).field(key_path).finish()
      }
      DepLocation::PackageJson(arc, key_path) => {
        let mut debug = f.debug_tuple("PackageJson");
        debug.field(&DebugAdapter(arc)).field(key_path).finish()
      }
    }
  }
}

#[derive(Clone, Debug)]
enum DepKind {
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
enum KeyPart {
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
  fn as_str(&self) -> &str {
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
struct KeyPath {
  keys: Vec<KeyPart>,
}

impl KeyPath {
  fn from_parts(parts: impl IntoIterator<Item = KeyPart>) -> Self {
    Self {
      keys: parts.into_iter().collect(),
    }
  }
  fn push(&mut self, part: KeyPart) {
    self.keys.push(part)
  }
}

#[derive(Clone, Debug)]
struct Dep {
  req: PackageReq,
  kind: DepKind,
  location: DepLocation,
  // specifier: String,
}

#[derive(Debug, Clone)]
struct ResolvedDep {
  dep: Dep,
  resolved_version: Option<Version>,
}

#[derive(Debug)]
struct WorkspaceDeps {
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
) -> Result<ImportMapWithDiagnostics, AnyError> {
  let mut map = serde_json::Map::with_capacity(2);
  if let Some(imports) = &deno_json.json.imports {
    map.insert("imports".to_string(), imports.clone());
  }
  if let Some(scopes) = &deno_json.json.scopes {
    map.insert("scopes".to_string(), scopes.clone());
  }
  import_map::parse_from_value(
    deno_json.specifier.clone(),
    serde_json::Value::Object(map),
  )
  .map_err(Into::into)
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

impl WorkspaceDeps {
  fn from_workspace(workspace: &Arc<Workspace>) -> Result<Self, AnyError> {
    let mut deps = Vec::new();
    for deno_json in workspace.deno_jsons() {
      let import_map = match deno_json_import_map(deno_json) {
        Ok(import_map) => import_map,
        Err(e) => {
          log::warn!(
            "failed to parse imports from {}: {e}",
            &deno_json.specifier
          );
          continue;
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
            log::warn!(
              "failed to parse package req \"{}\": {err}",
              value.as_str()
            );
            continue;
          }
        };
        deps.push(Dep {
          location: DepLocation::DenoJson(deno_json.clone(), key_path),
          kind,
          req: req.req,
        })
      }
    }
    for package_json in workspace.package_jsons() {
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
          deno_package_json::PackageJsonDepValue::Req(package_req) => deps
            .push(Dep {
              kind: DepKind::Npm,
              location: DepLocation::PackageJson(
                package_json.clone(),
                KeyPath::from_parts([package_dep_kind.into(), k.into()]),
              ),
              req: package_req,
            }),
          deno_package_json::PackageJsonDepValue::Workspace(_) => continue,
        }
      }
    }
    Ok(Self { deps })
  }
}

pub async fn outdated(
  flags: Arc<Flags>,
  outdated_flags: OutdatedFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  let cli_options = factory.cli_options()?;
  let workspace = cli_options.workspace();
  let resolver = factory.workspace_resolver().await?;
  let http_client = factory.http_client_provider();
  let deps_http_cache = factory.global_http_cache()?;
  let mut file_fetcher = FileFetcher::new(
    deps_http_cache.clone(),
    CacheSetting::ReloadAll,
    true,
    http_client.clone(),
    Default::default(),
    None,
  );
  file_fetcher.set_download_log_level(log::Level::Trace);
  let file_fetcher = Arc::new(file_fetcher);
  let deps_provider = NpmInstallDepsProvider::from_workspace(workspace);
  let npm_resolver =
    NpmFetchResolver::new(file_fetcher.clone(), cli_options.npmrc().clone());
  let jsr_resolver = JsrFetchResolver::new(file_fetcher.clone());

  let deps = WorkspaceDeps::from_workspace(workspace)?;

  let mut graph_permit = factory
    .main_module_graph_container()
    .await?
    .acquire_update_permit()
    .await;
  let root_permissions = factory.root_permissions_container()?;

  let graph = graph_permit.graph_mut();
  factory
    .module_load_preparer()
    .await?
    .prepare_module_load(
      graph,
      &[],
      false,
      deno_config::deno_json::TsTypeLib::DenoWindow,
      root_permissions.clone(),
      None,
    )
    .await?;

  eprintln!("deps: {deps:#?}");

  let reqs = graph.packages.mappings();

  let real_npm_resolver = factory.npm_resolver().await?;
  let snapshot = real_npm_resolver.as_managed().unwrap().snapshot();

  eprintln!("reqs: {reqs:?}");

  let mut resolved_deps = Vec::with_capacity(deps.deps.len());

  for dep in deps.deps {
    eprintln!("looking up {}", dep.req);
    match dep.kind {
      DepKind::Npm => {
        let nv = snapshot.package_reqs().get(&dep.req).unwrap();
        eprintln!("npm:{} => {nv}", dep.req);
        resolved_deps.push(ResolvedDep {
          dep,
          resolved_version: Some(nv.version.clone()),
        });
      }
      DepKind::Jsr => {
        let nv = reqs.get(&dep.req).unwrap();
        eprintln!("jsr:{} => {nv}", dep.req);
        resolved_deps.push(ResolvedDep {
          dep,
          resolved_version: Some(nv.version.clone()),
        });
      }
    }
  }

  let sema = Semaphore::new(32);

  let mut package_futs = FuturesUnordered::new();

  for resolved in resolved_deps {
    match resolved.dep.kind {
      DepKind::Npm => {
        package_futs.push(
          async {
            let _permit = sema.acquire().await.unwrap();
            let req = if outdated_flags.compatible {
              resolved.dep.req.clone()
            } else {
              PackageReq {
                name: resolved.dep.req.name.clone(),
                version_req: VersionReq::from_raw_text_and_inner(
                  "latest".into(),
                  deno_semver::RangeSetOrTag::Tag("latest".into()),
                ),
              }
            };
            let nv = npm_resolver.req_to_nv(&req).await;
            (resolved, nv)
          }
          .boxed(),
        );
      }
      DepKind::Jsr => {
        package_futs.push(
          async {
            let _permit = sema.acquire().await.unwrap();
            let req = if outdated_flags.compatible {
              resolved.dep.req.clone()
            } else {
              PackageReq {
                name: resolved.dep.req.name.clone(),
                version_req: deno_semver::WILDCARD_VERSION_REQ.clone(),
              }
            };
            let nv = jsr_resolver.req_to_nv(&req).await;
            (resolved, nv)
          }
          .boxed(),
        );
      }
    };
  }

  while let Some((resolved, latest)) = package_futs.next().await {
    let Some(latest) = latest else {
      continue;
    };
    let resolved_version = resolved.resolved_version.unwrap_or_default();
    if latest.version > resolved_version {
      eprintln!(
        "Outdated package {} : have {}, latest {}",
        resolved.dep.req, resolved_version, latest.version
      );
    }
  }

  Ok(())
}
