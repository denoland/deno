// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_cache_dir::file_fetcher::CacheSetting;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_path_util::url_to_file_path;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::VersionReq;
use deps::KeyPath;
use jsonc_parser::cst::CstObject;
use jsonc_parser::cst::CstObjectProp;
use jsonc_parser::cst::CstRootNode;
use jsonc_parser::json;

use crate::args::AddFlags;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::RemoveFlags;
use crate::factory::CliFactory;
use crate::file_fetcher::CliFileFetcher;
use crate::jsr::JsrFetchResolver;
use crate::npm::NpmFetchResolver;

mod cache_deps;
pub(crate) mod deps;
mod outdated;

pub use cache_deps::cache_top_level_deps;
pub use outdated::outdated;

#[derive(Debug, Copy, Clone, Hash)]
enum ConfigKind {
  DenoJson,
  PackageJson,
}

struct ConfigUpdater {
  kind: ConfigKind,
  cst: CstRootNode,
  root_object: CstObject,
  path: PathBuf,
  modified: bool,
}

impl ConfigUpdater {
  fn new(
    kind: ConfigKind,
    config_file_path: PathBuf,
  ) -> Result<Self, AnyError> {
    let config_file_contents = std::fs::read_to_string(&config_file_path)
      .with_context(|| {
        format!("Reading config file '{}'", config_file_path.display())
      })?;
    let cst = CstRootNode::parse(&config_file_contents, &Default::default())
      .with_context(|| {
        format!("Parsing config file '{}'", config_file_path.display())
      })?;
    let root_object = cst.object_value_or_set();
    Ok(Self {
      kind,
      cst,
      root_object,
      path: config_file_path,
      modified: false,
    })
  }

  fn display_path(&self) -> String {
    deno_path_util::url_from_file_path(&self.path)
      .map(|u| u.to_string())
      .unwrap_or_else(|_| self.path.display().to_string())
  }

  fn obj(&self) -> &CstObject {
    &self.root_object
  }

  fn contents(&self) -> String {
    self.cst.to_string()
  }

  fn get_property_for_mutation(
    &mut self,
    key_path: &KeyPath,
  ) -> Option<CstObjectProp> {
    let mut current_node = self.root_object.clone();

    self.modified = true;

    for (i, part) in key_path.parts.iter().enumerate() {
      let s = part.as_str();
      if i < key_path.parts.len().saturating_sub(1) {
        let object = current_node.object_value(s)?;
        current_node = object;
      } else {
        // last part
        return current_node.get(s);
      }
    }

    None
  }

  fn add(&mut self, selected: SelectedPackage, dev: bool) {
    fn insert_index(object: &CstObject, searching_name: &str) -> usize {
      object
        .properties()
        .into_iter()
        .take_while(|prop| {
          let prop_name =
            prop.name().and_then(|name| name.decoded_value().ok());
          match prop_name {
            Some(current_name) => {
              searching_name.cmp(&current_name) == std::cmp::Ordering::Greater
            }
            None => true,
          }
        })
        .count()
    }

    match self.kind {
      ConfigKind::DenoJson => {
        let imports = self.root_object.object_value_or_set("imports");
        let value =
          format!("{}@{}", selected.package_name, selected.version_req);
        if let Some(prop) = imports.get(&selected.import_name) {
          prop.set_value(json!(value));
        } else {
          let index = insert_index(&imports, &selected.import_name);
          imports.insert(index, &selected.import_name, json!(value));
        }
      }
      ConfigKind::PackageJson => {
        let deps_prop = self.root_object.get("dependencies");
        let dev_deps_prop = self.root_object.get("devDependencies");

        let dependencies = if dev {
          self
            .root_object
            .object_value("devDependencies")
            .unwrap_or_else(|| {
              let index = deps_prop
                .as_ref()
                .map(|p| p.property_index() + 1)
                .unwrap_or_else(|| self.root_object.properties().len());
              self
                .root_object
                .insert(index, "devDependencies", json!({}))
                .object_value_or_set()
            })
        } else {
          self
            .root_object
            .object_value("dependencies")
            .unwrap_or_else(|| {
              let index = dev_deps_prop
                .as_ref()
                .map(|p| p.property_index())
                .unwrap_or_else(|| self.root_object.properties().len());
              self
                .root_object
                .insert(index, "dependencies", json!({}))
                .object_value_or_set()
            })
        };
        let other_dependencies = if dev {
          deps_prop.and_then(|p| p.value().and_then(|v| v.as_object()))
        } else {
          dev_deps_prop.and_then(|p| p.value().and_then(|v| v.as_object()))
        };

        let (alias, value) = package_json_dependency_entry(selected);

        if let Some(other) = other_dependencies {
          if let Some(prop) = other.get(&alias) {
            remove_prop_and_maybe_parent_prop(prop);
          }
        }

        if let Some(prop) = dependencies.get(&alias) {
          prop.set_value(json!(value));
        } else {
          let index = insert_index(&dependencies, &alias);
          dependencies.insert(index, &alias, json!(value));
        }
      }
    }

    self.modified = true;
  }

  fn remove(&mut self, package: &str) -> bool {
    let removed = match self.kind {
      ConfigKind::DenoJson => {
        if let Some(prop) = self
          .root_object
          .object_value("imports")
          .and_then(|i| i.get(package))
        {
          remove_prop_and_maybe_parent_prop(prop);
          true
        } else {
          false
        }
      }
      ConfigKind::PackageJson => {
        let deps = [
          self
            .root_object
            .object_value("dependencies")
            .and_then(|deps| deps.get(package)),
          self
            .root_object
            .object_value("devDependencies")
            .and_then(|deps| deps.get(package)),
        ];
        let removed = deps.iter().any(|d| d.is_some());
        for dep in deps.into_iter().flatten() {
          remove_prop_and_maybe_parent_prop(dep);
        }
        removed
      }
    };
    if removed {
      self.modified = true;
    }
    removed
  }

  fn commit(&self) -> Result<(), AnyError> {
    if !self.modified {
      return Ok(());
    }

    let new_text = self.contents();
    std::fs::write(&self.path, new_text).with_context(|| {
      format!("failed writing to '{}'", self.path.display())
    })?;
    Ok(())
  }
}

fn remove_prop_and_maybe_parent_prop(prop: CstObjectProp) {
  let parent = prop.parent().unwrap().as_object().unwrap();
  prop.remove();
  if parent.properties().is_empty() {
    let parent_property = parent.parent().unwrap();
    let root_object = parent_property.parent().unwrap().as_object().unwrap();
    // remove the property
    parent_property.remove();
    root_object.ensure_multiline();
  }
}

fn create_deno_json(
  flags: &Arc<Flags>,
  options: &CliOptions,
) -> Result<CliFactory, AnyError> {
  std::fs::write(options.initial_cwd().join("deno.json"), "{}\n")
    .context("Failed to create deno.json file")?;
  log::info!("Created deno.json configuration file.");
  let factory = CliFactory::from_flags(flags.clone());
  Ok(factory)
}

fn package_json_dependency_entry(
  selected: SelectedPackage,
) -> (String, String) {
  if let Some(npm_package) = selected.package_name.strip_prefix("npm:") {
    if selected.import_name == npm_package {
      (npm_package.into(), selected.version_req)
    } else {
      (
        selected.import_name.into_string(),
        format!("npm:{}@{}", npm_package, selected.version_req),
      )
    }
  } else if let Some(jsr_package) = selected.package_name.strip_prefix("jsr:") {
    let jsr_package = jsr_package.strip_prefix('@').unwrap_or(jsr_package);
    let scope_replaced = jsr_package.replace('/', "__");
    let version_req =
      format!("npm:@jsr/{scope_replaced}@{}", selected.version_req);
    (selected.import_name.into_string(), version_req)
  } else {
    (selected.package_name, selected.version_req)
  }
}

#[derive(Clone, Copy)]
/// The name of the subcommand invoking the `add` operation.
pub enum AddCommandName {
  Add,
  Install,
}

impl std::fmt::Display for AddCommandName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      AddCommandName::Add => write!(f, "add"),
      AddCommandName::Install => write!(f, "install"),
    }
  }
}

fn load_configs(
  flags: &Arc<Flags>,
  has_jsr_specifiers: impl FnOnce() -> bool,
) -> Result<(CliFactory, Option<ConfigUpdater>, Option<ConfigUpdater>), AnyError>
{
  let cli_factory = CliFactory::from_flags(flags.clone());
  let options = cli_factory.cli_options()?;
  let start_dir = &options.start_dir;
  let npm_config = match start_dir.maybe_pkg_json() {
    Some(pkg_json) => Some(ConfigUpdater::new(
      ConfigKind::PackageJson,
      pkg_json.path.clone(),
    )?),
    None => None,
  };
  let deno_config = match start_dir.maybe_deno_json() {
    Some(deno_json) => Some(ConfigUpdater::new(
      ConfigKind::DenoJson,
      url_to_file_path(&deno_json.specifier)?,
    )?),
    None => None,
  };

  let (cli_factory, deno_config) = match deno_config {
    Some(config) => (cli_factory, Some(config)),
    None if npm_config.is_some() && !has_jsr_specifiers() => {
      (cli_factory, None)
    }
    _ => {
      let factory = create_deno_json(flags, options)?;
      let options = factory.cli_options()?.clone();
      let deno_json = options
        .start_dir
        .maybe_deno_json()
        .expect("Just created deno.json");
      (
        factory,
        Some(ConfigUpdater::new(
          ConfigKind::DenoJson,
          url_to_file_path(&deno_json.specifier)?,
        )?),
      )
    }
  };
  assert!(deno_config.is_some() || npm_config.is_some());
  Ok((cli_factory, npm_config, deno_config))
}

fn path_distance(a: &Path, b: &Path) -> usize {
  let diff = pathdiff::diff_paths(a, b);
  let Some(diff) = diff else {
    return usize::MAX;
  };
  diff.components().count()
}

pub async fn add(
  flags: Arc<Flags>,
  add_flags: AddFlags,
  cmd_name: AddCommandName,
) -> Result<(), AnyError> {
  let (cli_factory, mut npm_config, mut deno_config) =
    load_configs(&flags, || {
      add_flags.packages.iter().any(|s| s.starts_with("jsr:"))
    })?;

  if let Some(deno) = &deno_config {
    if deno.obj().get("importMap").is_some() {
      bail!(
        concat!(
          "`deno {}` is not supported when configuration file contains an \"importMap\" field. ",
          "Inline the import map into the Deno configuration file.\n",
          "    at {}",
        ),
        cmd_name,
        deno.display_path(),
      );
    }
  }

  let start_dir = cli_factory.cli_options()?.start_dir.dir_path();

  // only prefer to add npm deps to `package.json` if there isn't a closer deno.json.
  // example: if deno.json is in the CWD and package.json is in the parent, we should add
  // npm deps to deno.json, since it's closer
  let prefer_npm_config = match (npm_config.as_ref(), deno_config.as_ref()) {
    (Some(npm), Some(deno)) => {
      let npm_distance = path_distance(&npm.path, &start_dir);
      let deno_distance = path_distance(&deno.path, &start_dir);
      npm_distance <= deno_distance
    }
    (Some(_), None) => true,
    (None, _) => false,
  };

  let http_client = cli_factory.http_client_provider();
  let deps_http_cache = cli_factory.global_http_cache()?;
  let deps_file_fetcher = CliFileFetcher::new(
    deps_http_cache.clone(),
    http_client.clone(),
    cli_factory.sys(),
    Default::default(),
    None,
    true,
    CacheSetting::ReloadAll,
    log::Level::Trace,
  );

  let npmrc = cli_factory.npmrc()?;

  let deps_file_fetcher = Arc::new(deps_file_fetcher);
  let jsr_resolver = Arc::new(JsrFetchResolver::new(deps_file_fetcher.clone()));
  let npm_resolver =
    Arc::new(NpmFetchResolver::new(deps_file_fetcher, npmrc.clone()));

  let mut selected_packages = Vec::with_capacity(add_flags.packages.len());
  let mut package_reqs = Vec::with_capacity(add_flags.packages.len());

  for entry_text in add_flags.packages.iter() {
    let req = AddRmPackageReq::parse(
      entry_text,
      add_flags.default_registry.map(|r| r.into()),
    )
    .with_context(|| format!("Failed to parse package: {}", entry_text))?;

    match req {
      Ok(add_req) => package_reqs.push(add_req),
      Err(package_req) => {
        if jsr_resolver.req_to_nv(&package_req).await.is_some() {
          bail!(
            "{entry_text} is missing a prefix. Did you mean `{}`?",
            crate::colors::yellow(format!("deno {cmd_name} jsr:{package_req}"))
          )
        } else if npm_resolver.req_to_nv(&package_req).await.is_some() {
          bail!(
            "{entry_text} is missing a prefix. Did you mean `{}`?",
            crate::colors::yellow(format!("deno {cmd_name} npm:{package_req}"))
          )
        } else {
          bail!(
            "{} was not found in either jsr or npm.",
            crate::colors::red(entry_text)
          );
        }
      }
    }
  }

  let package_futures = package_reqs
    .into_iter()
    .map({
      let jsr_resolver = jsr_resolver.clone();
      move |package_req| {
        find_package_and_select_version_for_req(
          jsr_resolver.clone(),
          npm_resolver.clone(),
          package_req,
        )
        .boxed_local()
      }
    })
    .collect::<Vec<_>>();

  let stream_of_futures = deno_core::futures::stream::iter(package_futures);
  let mut buffered = stream_of_futures.buffered(10);

  while let Some(package_and_version_result) = buffered.next().await {
    let package_and_version = package_and_version_result?;

    match package_and_version {
      PackageAndVersion::NotFound {
        package: package_name,
        help,
        package_req,
      } => match help {
        Some(NotFoundHelp::NpmPackage) => {
          bail!(
            "{} was not found, but a matching npm package exists. Did you mean `{}`?",
            crate::colors::red(package_name),
            crate::colors::yellow(format!("deno {cmd_name} npm:{package_req}"))
          );
        }
        Some(NotFoundHelp::JsrPackage) => {
          bail!(
            "{} was not found, but a matching jsr package exists. Did you mean `{}`?",
            crate::colors::red(package_name),
            crate::colors::yellow(format!("deno {cmd_name} jsr:{package_req}"))
          )
        }
        Some(NotFoundHelp::PreReleaseVersion(version)) => {
          bail!(
            "{} has only pre-release versions available. Try specifying a version: `{}`",
            crate::colors::red(&package_name),
            crate::colors::yellow(format!("deno {cmd_name} {package_name}@^{version}"))
          )
        }
        None => bail!("{} was not found.", crate::colors::red(package_name)),
      },
      PackageAndVersion::Selected(selected) => {
        selected_packages.push(selected);
      }
    }
  }

  let dev = add_flags.dev;
  for selected_package in selected_packages {
    log::info!(
      "Add {}{}{}",
      crate::colors::green(&selected_package.package_name),
      crate::colors::gray("@"),
      selected_package.selected_version
    );

    if selected_package.package_name.starts_with("npm:") && prefer_npm_config {
      if let Some(npm) = &mut npm_config {
        npm.add(selected_package, dev);
      } else {
        deno_config.as_mut().unwrap().add(selected_package, dev);
      }
    } else if let Some(deno) = &mut deno_config {
      deno.add(selected_package, dev);
    } else {
      npm_config.as_mut().unwrap().add(selected_package, dev);
    }
  }

  if let Some(npm) = npm_config {
    npm.commit()?;
  }
  if let Some(deno) = deno_config {
    deno.commit()?;
  }

  npm_install_after_modification(flags, Some(jsr_resolver)).await?;

  Ok(())
}

struct SelectedPackage {
  import_name: StackString,
  package_name: String,
  version_req: String,
  selected_version: StackString,
}

enum NotFoundHelp {
  NpmPackage,
  JsrPackage,
  PreReleaseVersion(Version),
}

enum PackageAndVersion {
  NotFound {
    package: String,
    package_req: PackageReq,
    help: Option<NotFoundHelp>,
  },
  Selected(SelectedPackage),
}

fn best_version<'a>(
  versions: impl Iterator<Item = &'a Version>,
) -> Option<&'a Version> {
  let mut maybe_best_version: Option<&Version> = None;
  for version in versions {
    let is_best_version = maybe_best_version
      .as_ref()
      .map(|best_version| (*best_version).cmp(version).is_lt())
      .unwrap_or(true);
    if is_best_version {
      maybe_best_version = Some(version);
    }
  }
  maybe_best_version
}

trait PackageInfoProvider {
  const SPECIFIER_PREFIX: &str;
  /// The help to return if a package is found by this provider
  const HELP: NotFoundHelp;
  async fn req_to_nv(&self, req: &PackageReq) -> Option<PackageNv>;
  async fn latest_version(&self, req: &PackageReq) -> Option<Version>;
}

impl PackageInfoProvider for Arc<JsrFetchResolver> {
  const HELP: NotFoundHelp = NotFoundHelp::JsrPackage;
  const SPECIFIER_PREFIX: &str = "jsr";
  async fn req_to_nv(&self, req: &PackageReq) -> Option<PackageNv> {
    (**self).req_to_nv(req).await
  }

  async fn latest_version(&self, req: &PackageReq) -> Option<Version> {
    let info = self.package_info(&req.name).await?;
    best_version(
      info
        .versions
        .iter()
        .filter(|(_, version_info)| !version_info.yanked)
        .map(|(version, _)| version),
    )
    .cloned()
  }
}

impl PackageInfoProvider for Arc<NpmFetchResolver> {
  const HELP: NotFoundHelp = NotFoundHelp::NpmPackage;
  const SPECIFIER_PREFIX: &str = "npm";
  async fn req_to_nv(&self, req: &PackageReq) -> Option<PackageNv> {
    (**self).req_to_nv(req).await
  }

  async fn latest_version(&self, req: &PackageReq) -> Option<Version> {
    let info = self.package_info(&req.name).await?;
    best_version(info.versions.keys()).cloned()
  }
}

async fn find_package_and_select_version_for_req(
  jsr_resolver: Arc<JsrFetchResolver>,
  npm_resolver: Arc<NpmFetchResolver>,
  add_package_req: AddRmPackageReq,
) -> Result<PackageAndVersion, AnyError> {
  async fn select<T: PackageInfoProvider, S: PackageInfoProvider>(
    main_resolver: T,
    fallback_resolver: S,
    add_package_req: AddRmPackageReq,
  ) -> Result<PackageAndVersion, AnyError> {
    let req = match &add_package_req.value {
      AddRmPackageReqValue::Jsr(req) => req,
      AddRmPackageReqValue::Npm(req) => req,
    };
    let prefixed_name = format!("{}:{}", T::SPECIFIER_PREFIX, req.name);
    let help_if_found_in_fallback = S::HELP;
    let Some(nv) = main_resolver.req_to_nv(req).await else {
      if fallback_resolver.req_to_nv(req).await.is_some() {
        // it's in the other registry
        return Ok(PackageAndVersion::NotFound {
          package: prefixed_name,
          help: Some(help_if_found_in_fallback),
          package_req: req.clone(),
        });
      }
      if req.version_req.version_text() == "*" {
        if let Some(pre_release_version) =
          main_resolver.latest_version(req).await
        {
          return Ok(PackageAndVersion::NotFound {
            package: prefixed_name,
            package_req: req.clone(),
            help: Some(NotFoundHelp::PreReleaseVersion(
              pre_release_version.clone(),
            )),
          });
        }
      }

      return Ok(PackageAndVersion::NotFound {
        package: prefixed_name,
        help: None,
        package_req: req.clone(),
      });
    };
    let range_symbol = if req.version_req.version_text().starts_with('~') {
      "~"
    } else if req.version_req.version_text() == nv.version.to_string() {
      ""
    } else {
      "^"
    };
    Ok(PackageAndVersion::Selected(SelectedPackage {
      import_name: add_package_req.alias,
      package_name: prefixed_name,
      version_req: format!("{}{}", range_symbol, &nv.version),
      selected_version: nv.version.to_custom_string::<StackString>(),
    }))
  }

  match &add_package_req.value {
    AddRmPackageReqValue::Jsr(_) => {
      select(jsr_resolver, npm_resolver, add_package_req).await
    }
    AddRmPackageReqValue::Npm(_) => {
      select(npm_resolver, jsr_resolver, add_package_req).await
    }
  }
}

#[derive(Debug, PartialEq, Eq)]
enum AddRmPackageReqValue {
  Jsr(PackageReq),
  Npm(PackageReq),
}

#[derive(Debug, PartialEq, Eq)]
pub struct AddRmPackageReq {
  alias: StackString,
  value: AddRmPackageReqValue,
}

#[derive(Debug, Clone, Copy)]
pub enum Prefix {
  Jsr,
  Npm,
}

impl From<crate::args::DefaultRegistry> for Prefix {
  fn from(registry: crate::args::DefaultRegistry) -> Self {
    match registry {
      crate::args::DefaultRegistry::Npm => Prefix::Npm,
      crate::args::DefaultRegistry::Jsr => Prefix::Jsr,
    }
  }
}
impl AddRmPackageReq {
  pub fn parse(
    entry_text: &str,
    default_prefix: Option<Prefix>,
  ) -> Result<Result<Self, PackageReq>, AnyError> {
    fn parse_prefix(text: &str) -> (Option<Prefix>, &str) {
      if let Some(text) = text.strip_prefix("jsr:") {
        (Some(Prefix::Jsr), text)
      } else if let Some(text) = text.strip_prefix("npm:") {
        (Some(Prefix::Npm), text)
      } else {
        (None, text)
      }
    }

    // parse the following:
    // - alias@npm:<package_name>
    // - other_alias@npm:<package_name>
    // - @alias/other@jsr:<package_name>
    fn parse_alias(entry_text: &str) -> Option<(&str, &str)> {
      for prefix in ["npm:", "jsr:"] {
        let Some(location) = entry_text.find(prefix) else {
          continue;
        };
        let prefix = &entry_text[..location];
        if let Some(alias) = prefix.strip_suffix('@') {
          return Some((alias, &entry_text[location..]));
        }
      }
      None
    }

    let (maybe_prefix, entry_text) = parse_prefix(entry_text);
    let maybe_prefix = maybe_prefix.or(default_prefix);
    let (prefix, maybe_alias, entry_text) = match maybe_prefix {
      Some(prefix) => (prefix, None, entry_text),
      None => match parse_alias(entry_text) {
        Some((alias, text)) => {
          let (maybe_prefix, entry_text) = parse_prefix(text);
          let maybe_prefix = maybe_prefix.or(default_prefix);
          if maybe_prefix.is_none() {
            return Ok(Err(PackageReq::from_str(entry_text)?));
          }

          (
            maybe_prefix.unwrap(),
            Some(StackString::from(alias)),
            entry_text,
          )
        }
        None => return Ok(Err(PackageReq::from_str(entry_text)?)),
      },
    };

    match prefix {
      Prefix::Jsr => {
        let req_ref =
          JsrPackageReqReference::from_str(&format!("jsr:{}", entry_text))?;
        let package_req = req_ref.into_inner().req;
        Ok(Ok(AddRmPackageReq {
          alias: maybe_alias.unwrap_or_else(|| package_req.name.clone()),
          value: AddRmPackageReqValue::Jsr(package_req),
        }))
      }
      Prefix::Npm => {
        let req_ref =
          NpmPackageReqReference::from_str(&format!("npm:{}", entry_text))?;
        let mut package_req = req_ref.into_inner().req;
        // deno_semver defaults to a version req of `*` if none is specified
        // we want to default to `latest` instead
        if package_req.version_req == *deno_semver::WILDCARD_VERSION_REQ
          && package_req.version_req.version_text() == "*"
          && !entry_text.contains("@*")
        {
          package_req.version_req = VersionReq::from_raw_text_and_inner(
            "latest".into(),
            deno_semver::RangeSetOrTag::Tag("latest".into()),
          );
        }
        Ok(Ok(AddRmPackageReq {
          alias: maybe_alias.unwrap_or_else(|| package_req.name.clone()),
          value: AddRmPackageReqValue::Npm(package_req),
        }))
      }
    }
  }
}

pub async fn remove(
  flags: Arc<Flags>,
  remove_flags: RemoveFlags,
) -> Result<(), AnyError> {
  let (_, npm_config, deno_config) = load_configs(&flags, || false)?;

  let mut configs = [npm_config, deno_config];

  let mut removed_packages = vec![];

  for package in &remove_flags.packages {
    let req = AddRmPackageReq::parse(package, None)
      .with_context(|| format!("Failed to parse package: {}", package))?;
    let mut parsed_pkg_name = None;
    for config in configs.iter_mut().flatten() {
      match &req {
        Ok(rm_pkg) => {
          if config.remove(&rm_pkg.alias) && parsed_pkg_name.is_none() {
            parsed_pkg_name = Some(rm_pkg.alias.clone());
          }
        }
        Err(pkg) => {
          // An alias or a package name without registry/version
          // constraints. Try to remove the package anyway.
          if config.remove(&pkg.name) && parsed_pkg_name.is_none() {
            parsed_pkg_name = Some(pkg.name.clone());
          }
        }
      }
    }
    if let Some(pkg) = parsed_pkg_name {
      removed_packages.push(pkg);
    }
  }

  if removed_packages.is_empty() {
    log::info!("No packages were removed");
  } else {
    for package in &removed_packages {
      log::info!("Removed {}", crate::colors::green(package));
    }
    for config in configs.into_iter().flatten() {
      config.commit()?;
    }

    npm_install_after_modification(flags, None).await?;
  }

  Ok(())
}

async fn npm_install_after_modification(
  flags: Arc<Flags>,
  // explicitly provided to prevent redownloading
  jsr_resolver: Option<Arc<crate::jsr::JsrFetchResolver>>,
) -> Result<CliFactory, AnyError> {
  // clear the previously cached package.json from memory before reloading it
  node_resolver::PackageJsonThreadLocalCache::clear();

  // make a new CliFactory to pick up the updated config file
  let cli_factory = CliFactory::from_flags(flags);
  // surface any errors in the package.json
  let npm_installer = cli_factory.npm_installer().await?;
  npm_installer.ensure_no_pkg_json_dep_errors()?;
  // npm install
  cache_deps::cache_top_level_deps(&cli_factory, jsr_resolver).await?;

  if let Some(lockfile) = cli_factory.maybe_lockfile().await? {
    lockfile.write_if_changed()?;
  }

  Ok(cli_factory)
}

#[cfg(test)]
mod test {
  use super::*;

  fn jsr_pkg_req(alias: &str, req: &str) -> AddRmPackageReq {
    AddRmPackageReq {
      alias: alias.into(),
      value: AddRmPackageReqValue::Jsr(PackageReq::from_str(req).unwrap()),
    }
  }

  fn npm_pkg_req(alias: &str, req: &str) -> AddRmPackageReq {
    AddRmPackageReq {
      alias: alias.into(),
      value: AddRmPackageReqValue::Npm(PackageReq::from_str(req).unwrap()),
    }
  }

  #[test]
  fn test_parse_add_package_req() {
    let cases = [
      (("jsr:foo", None), jsr_pkg_req("foo", "foo")),
      (("alias@jsr:foo", None), jsr_pkg_req("alias", "foo")),
      (
        ("@alias/pkg@npm:foo", None),
        npm_pkg_req("@alias/pkg", "foo@latest"),
      ),
      (
        ("@alias/pkg@jsr:foo", None),
        jsr_pkg_req("@alias/pkg", "foo"),
      ),
      (
        ("alias@jsr:foo@^1.5.0", None),
        jsr_pkg_req("alias", "foo@^1.5.0"),
      ),
      (("foo", Some(Prefix::Npm)), npm_pkg_req("foo", "foo@latest")),
      (("foo", Some(Prefix::Jsr)), jsr_pkg_req("foo", "foo")),
      (
        ("npm:foo", Some(Prefix::Npm)),
        npm_pkg_req("foo", "foo@latest"),
      ),
      (("jsr:foo", Some(Prefix::Jsr)), jsr_pkg_req("foo", "foo")),
      (
        ("npm:foo", Some(Prefix::Jsr)),
        npm_pkg_req("foo", "foo@latest"),
      ),
      (("jsr:foo", Some(Prefix::Npm)), jsr_pkg_req("foo", "foo")),
    ];

    for ((input, maybe_prefix), expected) in cases {
      let s = format!("on input: {input}, maybe_prefix: {maybe_prefix:?}");
      assert_eq!(
        AddRmPackageReq::parse(input, maybe_prefix)
          .expect(&s)
          .expect(&s),
        expected,
        "{s}",
      );
    }

    assert_eq!(
      AddRmPackageReq::parse("@scope/pkg@tag", None)
        .unwrap()
        .unwrap_err()
        .to_string(),
      "@scope/pkg@tag",
    );
  }
}
