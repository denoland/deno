// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

mod cache_deps;

pub use cache_deps::cache_top_level_deps;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::VersionReq;

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::TextChange;
use deno_config::deno_json::FmtOptionsConfig;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use deno_runtime::deno_node;
use deno_semver::package::PackageReq;
use indexmap::IndexMap;
use jsonc_parser::ast::ObjectProp;
use jsonc_parser::ast::Value;
use yoke::Yoke;

use crate::args::AddFlags;
use crate::args::CacheSetting;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::RemoveFlags;
use crate::factory::CliFactory;
use crate::file_fetcher::FileFetcher;
use crate::jsr::JsrFetchResolver;
use crate::npm::NpmFetchResolver;

enum DenoConfigFormat {
  Json,
  Jsonc,
}

impl DenoConfigFormat {
  fn from_specifier(spec: &ModuleSpecifier) -> Result<Self, AnyError> {
    let file_name = spec
      .path_segments()
      .ok_or_else(|| anyhow!("Empty path in deno config specifier: {spec}"))?
      .last()
      .unwrap();
    match file_name {
      "deno.json" => Ok(Self::Json),
      "deno.jsonc" => Ok(Self::Jsonc),
      _ => bail!("Unsupported deno config file: {file_name}"),
    }
  }
}

struct DenoConfig {
  config: Arc<deno_config::deno_json::ConfigFile>,
  format: DenoConfigFormat,
  imports: IndexMap<String, String>,
}

fn deno_json_imports(
  config: &deno_config::deno_json::ConfigFile,
) -> Result<IndexMap<String, String>, AnyError> {
  Ok(
    config
      .json
      .imports
      .clone()
      .map(|imports| {
        serde_json::from_value(imports)
          .map_err(|err| anyhow!("Malformed \"imports\" configuration: {err}"))
      })
      .transpose()?
      .unwrap_or_default(),
  )
}
impl DenoConfig {
  fn from_options(options: &CliOptions) -> Result<Option<Self>, AnyError> {
    let start_dir = &options.start_dir;
    if let Some(config) = start_dir.maybe_deno_json() {
      Ok(Some(Self {
        imports: deno_json_imports(config)?,
        config: config.clone(),
        format: DenoConfigFormat::from_specifier(&config.specifier)?,
      }))
    } else {
      Ok(None)
    }
  }

  fn add(&mut self, selected: SelectedPackage) {
    self.imports.insert(
      selected.import_name,
      format!("{}@{}", selected.package_name, selected.version_req),
    );
  }

  fn remove(&mut self, package: &str) -> bool {
    self.imports.shift_remove(package).is_some()
  }

  fn take_import_fields(
    &mut self,
  ) -> Vec<(&'static str, IndexMap<String, String>)> {
    vec![("imports", std::mem::take(&mut self.imports))]
  }
}

impl NpmConfig {
  fn from_options(options: &CliOptions) -> Result<Option<Self>, AnyError> {
    let start_dir = &options.start_dir;
    if let Some(pkg_json) = start_dir.maybe_pkg_json() {
      Ok(Some(Self {
        dependencies: pkg_json.dependencies.clone().unwrap_or_default(),
        dev_dependencies: pkg_json.dev_dependencies.clone().unwrap_or_default(),
        config: pkg_json.clone(),
        fmt_options: None,
      }))
    } else {
      Ok(None)
    }
  }

  fn add(&mut self, selected: SelectedPackage, dev: bool) {
    let (name, version) = package_json_dependency_entry(selected);
    if dev {
      self.dependencies.swap_remove(&name);
      self.dev_dependencies.insert(name, version);
    } else {
      self.dev_dependencies.swap_remove(&name);
      self.dependencies.insert(name, version);
    }
  }

  fn remove(&mut self, package: &str) -> bool {
    let in_deps = self.dependencies.shift_remove(package).is_some();
    let in_dev_deps = self.dev_dependencies.shift_remove(package).is_some();
    in_deps || in_dev_deps
  }

  fn take_import_fields(
    &mut self,
  ) -> Vec<(&'static str, IndexMap<String, String>)> {
    vec![
      ("dependencies", std::mem::take(&mut self.dependencies)),
      (
        "devDependencies",
        std::mem::take(&mut self.dev_dependencies),
      ),
    ]
  }
}

struct NpmConfig {
  config: Arc<deno_node::PackageJson>,
  fmt_options: Option<FmtOptionsConfig>,
  dependencies: IndexMap<String, String>,
  dev_dependencies: IndexMap<String, String>,
}

enum DenoOrPackageJson {
  Deno(DenoConfig),
  Npm(NpmConfig),
}

impl From<DenoConfig> for DenoOrPackageJson {
  fn from(config: DenoConfig) -> Self {
    Self::Deno(config)
  }
}

impl From<NpmConfig> for DenoOrPackageJson {
  fn from(config: NpmConfig) -> Self {
    Self::Npm(config)
  }
}

/// Wrapper around `jsonc_parser::ast::Object` that can be stored in a `Yoke`
#[derive(yoke::Yokeable)]
struct JsoncObjectView<'a>(jsonc_parser::ast::Object<'a>);

struct ConfigUpdater {
  config: DenoOrPackageJson,
  // the `Yoke` is so we can carry the parsed object (which borrows from
  // the source) along with the source itself
  ast: Yoke<JsoncObjectView<'static>, String>,
  path: PathBuf,
  modified: bool,
}

impl ConfigUpdater {
  fn obj(&self) -> &jsonc_parser::ast::Object<'_> {
    &self.ast.get().0
  }
  fn contents(&self) -> &str {
    self.ast.backing_cart()
  }
  async fn maybe_new(
    config: Option<impl Into<DenoOrPackageJson>>,
  ) -> Result<Option<Self>, AnyError> {
    if let Some(config) = config {
      Ok(Some(Self::new(config.into()).await?))
    } else {
      Ok(None)
    }
  }
  async fn new(config: DenoOrPackageJson) -> Result<Self, AnyError> {
    let specifier = config.specifier();
    if specifier.scheme() != "file" {
      bail!("Can't update a remote configuration file");
    }
    let config_file_path = specifier.to_file_path().map_err(|_| {
      anyhow!("Specifier {specifier:?} is an invalid file path")
    })?;
    let config_file_contents = {
      let contents = tokio::fs::read_to_string(&config_file_path)
        .await
        .with_context(|| {
          format!("Reading config file at: {}", config_file_path.display())
        })?;
      if contents.trim().is_empty() {
        "{}\n".into()
      } else {
        contents
      }
    };
    let ast = Yoke::try_attach_to_cart(config_file_contents, |contents| {
      let ast = jsonc_parser::parse_to_ast(
        contents,
        &Default::default(),
        &Default::default(),
      )
      .with_context(|| {
        format!("Failed to parse config file at {}", specifier)
      })?;
      let obj = match ast.value {
        Some(Value::Object(obj)) => obj,
        _ => bail!(
          "Failed to update config file at {}, expected an object",
          specifier
        ),
      };
      Ok(JsoncObjectView(obj))
    })?;
    Ok(Self {
      config,
      ast,
      path: config_file_path,
      modified: false,
    })
  }

  fn add(&mut self, selected: SelectedPackage, dev: bool) {
    match &mut self.config {
      DenoOrPackageJson::Deno(deno) => deno.add(selected),
      DenoOrPackageJson::Npm(npm) => npm.add(selected, dev),
    }
    self.modified = true;
  }

  fn remove(&mut self, package: &str) -> bool {
    let removed = match &mut self.config {
      DenoOrPackageJson::Deno(deno) => deno.remove(package),
      DenoOrPackageJson::Npm(npm) => npm.remove(package),
    };
    if removed {
      self.modified = true;
    }
    removed
  }

  async fn commit(mut self) -> Result<(), AnyError> {
    if !self.modified {
      return Ok(());
    }

    let import_fields = self.config.take_import_fields();

    let fmt_config_options = self.config.fmt_options();

    let new_text = update_config_file_content(
      self.obj(),
      self.contents(),
      fmt_config_options,
      import_fields.into_iter().map(|(k, v)| {
        (
          k,
          if v.is_empty() {
            None
          } else {
            Some(generate_imports(v.into_iter().collect()))
          },
        )
      }),
      self.config.file_name(),
    );

    tokio::fs::write(&self.path, new_text).await?;
    Ok(())
  }
}

impl DenoOrPackageJson {
  fn specifier(&self) -> Cow<ModuleSpecifier> {
    match self {
      Self::Deno(d, ..) => Cow::Borrowed(&d.config.specifier),
      Self::Npm(n, ..) => Cow::Owned(n.config.specifier()),
    }
  }

  fn fmt_options(&self) -> FmtOptionsConfig {
    match self {
      DenoOrPackageJson::Deno(deno, ..) => deno
        .config
        .to_fmt_config()
        .ok()
        .map(|f| f.options)
        .unwrap_or_default(),
      DenoOrPackageJson::Npm(config) => {
        config.fmt_options.clone().unwrap_or_default()
      }
    }
  }

  fn take_import_fields(
    &mut self,
  ) -> Vec<(&'static str, IndexMap<String, String>)> {
    match self {
      Self::Deno(d) => d.take_import_fields(),
      Self::Npm(n) => n.take_import_fields(),
    }
  }

  fn file_name(&self) -> &'static str {
    match self {
      DenoOrPackageJson::Deno(config) => match config.format {
        DenoConfigFormat::Json => "deno.json",
        DenoConfigFormat::Jsonc => "deno.jsonc",
      },
      DenoOrPackageJson::Npm(..) => "package.json",
    }
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
    (npm_package.into(), selected.version_req)
  } else if let Some(jsr_package) = selected.package_name.strip_prefix("jsr:") {
    let jsr_package = jsr_package.strip_prefix('@').unwrap_or(jsr_package);
    let scope_replaced = jsr_package.replace('/', "__");
    let version_req =
      format!("npm:@jsr/{scope_replaced}@{}", selected.version_req);
    (selected.import_name, version_req)
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
) -> Result<(CliFactory, Option<NpmConfig>, Option<DenoConfig>), AnyError> {
  let cli_factory = CliFactory::from_flags(flags.clone());
  let options = cli_factory.cli_options()?;
  let npm_config = NpmConfig::from_options(options)?;
  let (cli_factory, deno_config) = match DenoConfig::from_options(options)? {
    Some(config) => (cli_factory, Some(config)),
    None if npm_config.is_some() => (cli_factory, None),
    None => {
      let factory = create_deno_json(flags, options)?;
      let options = factory.cli_options()?.clone();
      (
        factory,
        Some(
          DenoConfig::from_options(&options)?.expect("Just created deno.json"),
        ),
      )
    }
  };
  assert!(deno_config.is_some() || npm_config.is_some());
  Ok((cli_factory, npm_config, deno_config))
}

pub async fn add(
  flags: Arc<Flags>,
  add_flags: AddFlags,
  cmd_name: AddCommandName,
) -> Result<(), AnyError> {
  let (cli_factory, npm_config, deno_config) = load_configs(&flags)?;
  let mut npm_config = ConfigUpdater::maybe_new(npm_config).await?;
  let mut deno_config = ConfigUpdater::maybe_new(deno_config).await?;

  if let Some(deno) = &deno_config {
    let specifier = deno.config.specifier();
    if deno.obj().get_string("importMap").is_some() {
      bail!(
        concat!(
          "`deno {}` is not supported when configuration file contains an \"importMap\" field. ",
          "Inline the import map into the Deno configuration file.\n",
          "    at {}",
        ),
        cmd_name,
        specifier
      );
    }
  }

  let http_client = cli_factory.http_client_provider();
  let deps_http_cache = cli_factory.global_http_cache()?;
  let mut deps_file_fetcher = FileFetcher::new(
    deps_http_cache.clone(),
    CacheSetting::ReloadAll,
    true,
    http_client.clone(),
    Default::default(),
    None,
  );
  deps_file_fetcher.set_download_log_level(log::Level::Trace);
  let deps_file_fetcher = Arc::new(deps_file_fetcher);
  let jsr_resolver = Arc::new(JsrFetchResolver::new(deps_file_fetcher.clone()));
  let npm_resolver = Arc::new(NpmFetchResolver::new(deps_file_fetcher));

  let mut selected_packages = Vec::with_capacity(add_flags.packages.len());
  let mut package_reqs = Vec::with_capacity(add_flags.packages.len());

  for entry_text in add_flags.packages.iter() {
    let req = AddPackageReq::parse(entry_text).with_context(|| {
      format!("Failed to parse package required: {}", entry_text)
    })?;

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
        found_npm_package,
        package_req,
      } => {
        if found_npm_package {
          bail!("{} was not found, but a matching npm package exists. Did you mean `{}`?", crate::colors::red(package_name), crate::colors::yellow(format!("deno {cmd_name} npm:{package_req}")));
        } else {
          bail!("{} was not found.", crate::colors::red(package_name));
        }
      }
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

    if selected_package.package_name.starts_with("npm:") {
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

  let mut commit_futures = vec![];
  if let Some(npm) = npm_config {
    commit_futures.push(npm.commit());
  }
  if let Some(deno) = deno_config {
    commit_futures.push(deno.commit());
  }
  let commit_futures =
    deno_core::futures::future::join_all(commit_futures).await;

  for result in commit_futures {
    result.context("Failed to update configuration file")?;
  }

  npm_install_after_modification(flags, Some(jsr_resolver)).await?;

  Ok(())
}

struct SelectedPackage {
  import_name: String,
  package_name: String,
  version_req: String,
  selected_version: String,
}

enum PackageAndVersion {
  NotFound {
    package: String,
    found_npm_package: bool,
    package_req: PackageReq,
  },
  Selected(SelectedPackage),
}

async fn find_package_and_select_version_for_req(
  jsr_resolver: Arc<JsrFetchResolver>,
  npm_resolver: Arc<NpmFetchResolver>,
  add_package_req: AddPackageReq,
) -> Result<PackageAndVersion, AnyError> {
  match add_package_req.value {
    AddPackageReqValue::Jsr(req) => {
      let jsr_prefixed_name = format!("jsr:{}", &req.name);
      let Some(nv) = jsr_resolver.req_to_nv(&req).await else {
        if npm_resolver.req_to_nv(&req).await.is_some() {
          return Ok(PackageAndVersion::NotFound {
            package: jsr_prefixed_name,
            found_npm_package: true,
            package_req: req,
          });
        }

        return Ok(PackageAndVersion::NotFound {
          package: jsr_prefixed_name,
          found_npm_package: false,
          package_req: req,
        });
      };
      let range_symbol = if req.version_req.version_text().starts_with('~') {
        '~'
      } else {
        '^'
      };
      Ok(PackageAndVersion::Selected(SelectedPackage {
        import_name: add_package_req.alias,
        package_name: jsr_prefixed_name,
        version_req: format!("{}{}", range_symbol, &nv.version),
        selected_version: nv.version.to_string(),
      }))
    }
    AddPackageReqValue::Npm(req) => {
      let npm_prefixed_name = format!("npm:{}", &req.name);
      let Some(nv) = npm_resolver.req_to_nv(&req).await else {
        return Ok(PackageAndVersion::NotFound {
          package: npm_prefixed_name,
          found_npm_package: false,
          package_req: req,
        });
      };
      let range_symbol = if req.version_req.version_text().starts_with('~') {
        '~'
      } else {
        '^'
      };
      Ok(PackageAndVersion::Selected(SelectedPackage {
        import_name: add_package_req.alias,
        package_name: npm_prefixed_name,
        version_req: format!("{}{}", range_symbol, &nv.version),
        selected_version: nv.version.to_string(),
      }))
    }
  }
}

#[derive(Debug, PartialEq, Eq)]
enum AddPackageReqValue {
  Jsr(PackageReq),
  Npm(PackageReq),
}

#[derive(Debug, PartialEq, Eq)]
struct AddPackageReq {
  alias: String,
  value: AddPackageReqValue,
}

impl AddPackageReq {
  pub fn parse(entry_text: &str) -> Result<Result<Self, PackageReq>, AnyError> {
    enum Prefix {
      Jsr,
      Npm,
    }

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
    let (prefix, maybe_alias, entry_text) = match maybe_prefix {
      Some(prefix) => (prefix, None, entry_text),
      None => match parse_alias(entry_text) {
        Some((alias, text)) => {
          let (maybe_prefix, entry_text) = parse_prefix(text);
          if maybe_prefix.is_none() {
            return Ok(Err(PackageReq::from_str(entry_text)?));
          }

          (maybe_prefix.unwrap(), Some(alias.to_string()), entry_text)
        }
        None => return Ok(Err(PackageReq::from_str(entry_text)?)),
      },
    };

    match prefix {
      Prefix::Jsr => {
        let req_ref =
          JsrPackageReqReference::from_str(&format!("jsr:{}", entry_text))?;
        let package_req = req_ref.into_inner().req;
        Ok(Ok(AddPackageReq {
          alias: maybe_alias.unwrap_or_else(|| package_req.name.to_string()),
          value: AddPackageReqValue::Jsr(package_req),
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
        Ok(Ok(AddPackageReq {
          alias: maybe_alias.unwrap_or_else(|| package_req.name.to_string()),
          value: AddPackageReqValue::Npm(package_req),
        }))
      }
    }
  }
}

fn generate_imports(mut packages_to_version: Vec<(String, String)>) -> String {
  packages_to_version.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
  let mut contents = vec![];
  let len = packages_to_version.len();
  for (index, (package, version)) in packages_to_version.iter().enumerate() {
    // TODO(bartlomieju): fix it, once we start support specifying version on the cli
    contents.push(format!("\"{}\": \"{}\"", package, version));
    if index != len - 1 {
      contents.push(",".to_string());
    }
  }
  contents.join("\n")
}

pub async fn remove(
  flags: Arc<Flags>,
  remove_flags: RemoveFlags,
) -> Result<(), AnyError> {
  let (_, npm_config, deno_config) = load_configs(&flags)?;

  let mut configs = [
    ConfigUpdater::maybe_new(npm_config).await?,
    ConfigUpdater::maybe_new(deno_config).await?,
  ];

  let mut removed_packages = vec![];

  for package in &remove_flags.packages {
    let mut removed = false;
    for config in configs.iter_mut().flatten() {
      removed |= config.remove(package);
    }
    if removed {
      removed_packages.push(package.clone());
    }
  }

  if removed_packages.is_empty() {
    log::info!("No packages were removed");
  } else {
    for package in &removed_packages {
      log::info!("Removed {}", crate::colors::green(package));
    }
    for config in configs.into_iter().flatten() {
      config.commit().await?;
    }

    npm_install_after_modification(flags, None).await?;
  }

  Ok(())
}

async fn npm_install_after_modification(
  flags: Arc<Flags>,
  // explicitly provided to prevent redownloading
  jsr_resolver: Option<Arc<crate::jsr::JsrFetchResolver>>,
) -> Result<(), AnyError> {
  // clear the previously cached package.json from memory before reloading it
  node_resolver::PackageJsonThreadLocalCache::clear();

  // make a new CliFactory to pick up the updated config file
  let cli_factory = CliFactory::from_flags(flags);
  // surface any errors in the package.json
  let npm_resolver = cli_factory.npm_resolver().await?;
  if let Some(npm_resolver) = npm_resolver.as_managed() {
    npm_resolver.ensure_no_pkg_json_dep_errors()?;
  }
  // npm install
  cache_deps::cache_top_level_deps(&cli_factory, jsr_resolver).await?;

  Ok(())
}

fn update_config_file_content<
  I: IntoIterator<Item = (&'static str, Option<String>)>,
>(
  obj: &jsonc_parser::ast::Object,
  config_file_contents: &str,
  fmt_options: FmtOptionsConfig,
  entries: I,
  file_name: &str,
) -> String {
  let mut text_changes = vec![];
  for (key, value) in entries {
    match obj.properties.iter().enumerate().find_map(|(idx, k)| {
      if k.name.as_str() == key {
        Some((idx, k))
      } else {
        None
      }
    }) {
      Some((
        idx,
        ObjectProp {
          value: Value::Object(lit),
          range,
          ..
        },
      )) => {
        if let Some(value) = value {
          text_changes.push(TextChange {
            range: (lit.range.start + 1)..(lit.range.end - 1),
            new_text: value,
          })
        } else {
          text_changes.push(TextChange {
            // remove field entirely, making sure to
            // remove the comma if it's not the last field
            range: range.start..(if idx == obj.properties.len() - 1 {
              range.end
            } else {
              obj.properties[idx + 1].range.start
            }),
            new_text: "".to_string(),
          })
        }
      }

      // need to add field
      None => {
        if let Some(value) = value {
          let insert_position = obj.range.end - 1;
          text_changes.push(TextChange {
            range: insert_position..insert_position,
            // NOTE(bartlomieju): adding `\n` here to force the formatter to always
            // produce a config file that is multiline, like so:
            // ```
            // {
            //   "imports": {
            //     "<package_name>": "<registry>:<package_name>@<semver>"
            //   }
            // }
            new_text: format!("\"{key}\": {{\n {value} }}"),
          })
        }
      }
      // we verified the shape of `imports`/`dependencies` above
      Some(_) => unreachable!(),
    }
  }

  let new_text =
    deno_ast::apply_text_changes(config_file_contents, text_changes);

  crate::tools::fmt::format_json(
    &PathBuf::from(file_name),
    &new_text,
    &fmt_options,
  )
  .ok()
  .map(|formatted_text| formatted_text.unwrap_or_else(|| new_text.clone()))
  .unwrap_or(new_text)
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_parse_add_package_req() {
    assert_eq!(
      AddPackageReq::parse("jsr:foo").unwrap().unwrap(),
      AddPackageReq {
        alias: "foo".to_string(),
        value: AddPackageReqValue::Jsr(PackageReq::from_str("foo").unwrap())
      }
    );
    assert_eq!(
      AddPackageReq::parse("alias@jsr:foo").unwrap().unwrap(),
      AddPackageReq {
        alias: "alias".to_string(),
        value: AddPackageReqValue::Jsr(PackageReq::from_str("foo").unwrap())
      }
    );
    assert_eq!(
      AddPackageReq::parse("@alias/pkg@npm:foo").unwrap().unwrap(),
      AddPackageReq {
        alias: "@alias/pkg".to_string(),
        value: AddPackageReqValue::Npm(
          PackageReq::from_str("foo@latest").unwrap()
        )
      }
    );
    assert_eq!(
      AddPackageReq::parse("@alias/pkg@jsr:foo").unwrap().unwrap(),
      AddPackageReq {
        alias: "@alias/pkg".to_string(),
        value: AddPackageReqValue::Jsr(PackageReq::from_str("foo").unwrap())
      }
    );
    assert_eq!(
      AddPackageReq::parse("alias@jsr:foo@^1.5.0")
        .unwrap()
        .unwrap(),
      AddPackageReq {
        alias: "alias".to_string(),
        value: AddPackageReqValue::Jsr(
          PackageReq::from_str("foo@^1.5.0").unwrap()
        )
      }
    );
    assert_eq!(
      AddPackageReq::parse("@scope/pkg@tag")
        .unwrap()
        .unwrap_err()
        .to_string(),
      "@scope/pkg@tag",
    );
  }
}
