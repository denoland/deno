// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::rc::Rc;
use std::sync::Arc;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use deno_ast::ModuleSpecifier;
use deno_config::workspace::JsrPackageConfig;
use deno_config::workspace::PackageJsonDepResolution;
use deno_config::workspace::Workspace;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::future::LocalBoxFuture;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_terminal::colors;
use http_body_util::BodyExt;
use lsp_types::Url;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use tokio::process::Command;

use crate::args::jsr_api_url;
use crate::args::jsr_url;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::PublishFlags;
use crate::cache::LazyGraphSourceParser;
use crate::cache::ParsedSourceCache;
use crate::factory::CliFactory;
use crate::graph_util::ModuleGraphCreator;
use crate::http_util::HttpClient;
use crate::resolver::SloppyImportsResolver;
use crate::tools::check::CheckOptions;
use crate::tools::lint::collect_no_slow_type_diagnostics;
use crate::tools::registry::diagnostics::PublishDiagnostic;
use crate::tools::registry::diagnostics::PublishDiagnosticsCollector;
use crate::util::display::human_size;

mod api;
mod auth;
mod diagnostics;
mod graph;
mod paths;
mod pm;
mod provenance;
mod publish_order;
mod tar;
mod unfurl;

use auth::get_auth_method;
use auth::AuthMethod;
pub use pm::add;
use publish_order::PublishOrderGraph;
use unfurl::SpecifierUnfurler;

use super::check::TypeChecker;

use self::graph::GraphDiagnosticsCollector;
use self::paths::CollectedPublishPath;
use self::tar::PublishableTarball;

pub async fn publish(
  flags: Arc<Flags>,
  publish_flags: PublishFlags,
) -> Result<(), AnyError> {
  let cli_factory = CliFactory::from_flags(flags);

  let auth_method =
    get_auth_method(publish_flags.token, publish_flags.dry_run)?;

  let cli_options = cli_factory.cli_options()?;
  let directory_path = cli_options.initial_cwd();
  let publish_configs = cli_options.start_dir.jsr_packages_for_publish();
  if publish_configs.is_empty() {
    match cli_options.start_dir.maybe_deno_json() {
      Some(deno_json) => {
        debug_assert!(!deno_json.is_package());
        bail!(
          "Missing 'name', 'version' and 'exports' field in '{}'.",
          deno_json.specifier
        );
      }
      None => {
        bail!(
          "Couldn't find a deno.json, deno.jsonc, jsr.json or jsr.jsonc configuration file in {}.",
          directory_path.display()
        );
      }
    }
  }
  let specifier_unfurler = Arc::new(SpecifierUnfurler::new(
    if cli_options.unstable_sloppy_imports() {
      Some(SloppyImportsResolver::new(cli_factory.fs().clone()))
    } else {
      None
    },
    cli_options
      .create_workspace_resolver(
        cli_factory.file_fetcher()?,
        PackageJsonDepResolution::Enabled,
      )
      .await?,
    cli_options.unstable_bare_node_builtins(),
  ));

  let diagnostics_collector = PublishDiagnosticsCollector::default();
  let publish_preparer = PublishPreparer::new(
    GraphDiagnosticsCollector::new(cli_factory.parsed_source_cache().clone()),
    cli_factory.module_graph_creator().await?.clone(),
    cli_factory.parsed_source_cache().clone(),
    cli_factory.type_checker().await?.clone(),
    cli_options.clone(),
    specifier_unfurler,
  );

  let prepared_data = publish_preparer
    .prepare_packages_for_publishing(
      publish_flags.allow_slow_types,
      &diagnostics_collector,
      publish_configs,
    )
    .await?;

  diagnostics_collector.print_and_error()?;

  if prepared_data.package_by_name.is_empty() {
    bail!("No packages to publish");
  }

  if std::env::var("DENO_TESTING_DISABLE_GIT_CHECK")
    .ok()
    .is_none()
    && !publish_flags.allow_dirty
  {
    if let Some(dirty_text) =
      check_if_git_repo_dirty(cli_options.initial_cwd()).await
    {
      log::error!("\nUncommitted changes:\n\n{}\n", dirty_text);
      bail!("Aborting due to uncommitted changes. Check in source code or run with --allow-dirty");
    }
  }

  if publish_flags.dry_run {
    for (_, package) in prepared_data.package_by_name {
      log::info!(
        "{} of {} with files:",
        colors::green_bold("Simulating publish"),
        colors::gray(package.display_name()),
      );
      for file in &package.tarball.files {
        log::info!("   {} ({})", file.specifier, human_size(file.size as f64),);
      }
    }
    log::warn!("{} Aborting due to --dry-run", colors::yellow("Warning"));
    return Ok(());
  }

  perform_publish(
    &cli_factory.http_client_provider().get_or_create()?,
    prepared_data.publish_order_graph,
    prepared_data.package_by_name,
    auth_method,
    !publish_flags.no_provenance,
  )
  .await?;

  Ok(())
}

struct PreparedPublishPackage {
  scope: String,
  package: String,
  version: String,
  tarball: PublishableTarball,
  config: String,
  exports: HashMap<String, String>,
}

impl PreparedPublishPackage {
  pub fn display_name(&self) -> String {
    format!("@{}/{}@{}", self.scope, self.package, self.version)
  }
}

struct PreparePackagesData {
  publish_order_graph: PublishOrderGraph,
  package_by_name: HashMap<String, Rc<PreparedPublishPackage>>,
}

struct PublishPreparer {
  graph_diagnostics_collector: GraphDiagnosticsCollector,
  module_graph_creator: Arc<ModuleGraphCreator>,
  source_cache: Arc<ParsedSourceCache>,
  type_checker: Arc<TypeChecker>,
  cli_options: Arc<CliOptions>,
  specifier_unfurler: Arc<SpecifierUnfurler>,
}

impl PublishPreparer {
  pub fn new(
    graph_diagnostics_collector: GraphDiagnosticsCollector,
    module_graph_creator: Arc<ModuleGraphCreator>,
    source_cache: Arc<ParsedSourceCache>,
    type_checker: Arc<TypeChecker>,
    cli_options: Arc<CliOptions>,
    specifier_unfurler: Arc<SpecifierUnfurler>,
  ) -> Self {
    Self {
      graph_diagnostics_collector,
      module_graph_creator,
      source_cache,
      type_checker,
      cli_options,
      specifier_unfurler,
    }
  }

  pub async fn prepare_packages_for_publishing(
    &self,
    allow_slow_types: bool,
    diagnostics_collector: &PublishDiagnosticsCollector,
    publish_configs: Vec<JsrPackageConfig>,
  ) -> Result<PreparePackagesData, AnyError> {
    if publish_configs.len() > 1 {
      log::info!("Publishing a workspace...");
    }

    // create the module graph
    let graph = self
      .build_and_check_graph_for_publish(
        allow_slow_types,
        diagnostics_collector,
        &publish_configs,
      )
      .await?;

    let mut package_by_name = HashMap::with_capacity(publish_configs.len());
    let publish_order_graph =
      publish_order::build_publish_order_graph(&graph, &publish_configs)?;

    let results = publish_configs
      .into_iter()
      .map(|member| {
        let graph = graph.clone();
        async move {
          let package = self
            .prepare_publish(&member, graph, diagnostics_collector)
            .await
            .with_context(|| format!("Failed preparing '{}'.", member.name))?;
          Ok::<_, AnyError>((member.name, package))
        }
        .boxed()
      })
      .collect::<Vec<_>>();
    let results = deno_core::futures::future::join_all(results).await;
    for result in results {
      let (package_name, package) = result?;
      package_by_name.insert(package_name, package);
    }
    Ok(PreparePackagesData {
      publish_order_graph,
      package_by_name,
    })
  }

  async fn build_and_check_graph_for_publish(
    &self,
    allow_slow_types: bool,
    diagnostics_collector: &PublishDiagnosticsCollector,
    package_configs: &[JsrPackageConfig],
  ) -> Result<Arc<deno_graph::ModuleGraph>, deno_core::anyhow::Error> {
    let build_fast_check_graph = !allow_slow_types;
    let graph = self
      .module_graph_creator
      .create_and_validate_publish_graph(
        package_configs,
        build_fast_check_graph,
      )
      .await?;

    // todo(dsherret): move to lint rule
    self
      .graph_diagnostics_collector
      .collect_diagnostics_for_graph(&graph, diagnostics_collector)?;

    if allow_slow_types {
      log::info!(
        concat!(
          "{} Publishing a library with slow types is not recommended. ",
          "This may lead to poor type checking performance for users of ",
          "your package, may affect the quality of automatic documentation ",
          "generation, and your package will not be shipped with a .d.ts ",
          "file for Node.js users."
        ),
        colors::yellow("Warning"),
      );
      Ok(Arc::new(graph))
    } else if std::env::var("DENO_INTERNAL_FAST_CHECK_OVERWRITE").as_deref()
      == Ok("1")
    {
      if check_if_git_repo_dirty(self.cli_options.initial_cwd())
        .await
        .is_some()
      {
        bail!("When using DENO_INTERNAL_FAST_CHECK_OVERWRITE, the git repo must be in a clean state.");
      }

      for module in graph.modules() {
        if module.specifier().scheme() != "file" {
          continue;
        }
        let Some(js) = module.js() else {
          continue;
        };
        if let Some(module) = js.fast_check_module() {
          std::fs::write(
            js.specifier.to_file_path().unwrap(),
            module.source.as_ref(),
          )?;
        }
      }

      bail!("Exiting due to DENO_INTERNAL_FAST_CHECK_OVERWRITE")
    } else {
      log::info!("Checking for slow types in the public API...");
      let mut any_pkg_had_diagnostics = false;
      for package in package_configs {
        let export_urls = package.config_file.resolve_export_value_urls()?;
        let diagnostics =
          collect_no_slow_type_diagnostics(&graph, &export_urls);
        if !diagnostics.is_empty() {
          any_pkg_had_diagnostics = true;
          for diagnostic in diagnostics {
            diagnostics_collector
              .push(PublishDiagnostic::FastCheck(diagnostic));
          }
        }
      }

      if any_pkg_had_diagnostics {
        Ok(Arc::new(graph))
      } else {
        // fast check passed, type check the output as a temporary measure
        // until we know that it's reliable and stable
        let (graph, check_diagnostics) = self
          .type_checker
          .check_diagnostics(
            graph,
            CheckOptions {
              build_fast_check_graph: false, // already built
              lib: self.cli_options.ts_type_lib_window(),
              log_ignored_options: false,
              reload: self.cli_options.reload_flag(),
              type_check_mode: self.cli_options.type_check_mode(),
            },
          )
          .await?;
        // ignore unused parameter diagnostics that may occur due to fast check
        // not having function body implementations
        let check_diagnostics =
          check_diagnostics.filter(|d| d.include_when_remote());
        if !check_diagnostics.is_empty() {
          bail!(
            concat!(
            "Failed ensuring public API type output is valid.\n\n",
            "{:#}\n\n",
            "You may have discovered a bug in Deno. Please open an issue at: ",
            "https://github.com/denoland/deno/issues/"
          ),
            check_diagnostics
          );
        }
        Ok(graph)
      }
    }
  }

  #[allow(clippy::too_many_arguments)]
  async fn prepare_publish(
    &self,
    package: &JsrPackageConfig,
    graph: Arc<deno_graph::ModuleGraph>,
    diagnostics_collector: &PublishDiagnosticsCollector,
  ) -> Result<Rc<PreparedPublishPackage>, AnyError> {
    static SUGGESTED_ENTRYPOINTS: [&str; 4] =
      ["mod.ts", "mod.js", "index.ts", "index.js"];

    let deno_json = &package.config_file;
    let config_path = deno_json.specifier.to_file_path().unwrap();
    let root_dir = config_path.parent().unwrap().to_path_buf();
    let Some(version) = deno_json.json.version.clone() else {
      bail!("{} is missing 'version' field", deno_json.specifier);
    };
    if deno_json.json.exports.is_none() {
      let mut suggested_entrypoint = None;

      for entrypoint in SUGGESTED_ENTRYPOINTS {
        if root_dir.join(entrypoint).exists() {
          suggested_entrypoint = Some(entrypoint);
          break;
        }
      }

      let exports_content = format!(
        r#"{{
  "name": "{}",
  "version": "{}",
  "exports": "{}"
}}"#,
        package.name,
        version,
        suggested_entrypoint.unwrap_or("<path_to_entrypoint>")
      );

      bail!(
      "You did not specify an entrypoint to \"{}\" package in {}. Add `exports` mapping in the configuration file, eg:\n{}",
      package.name,
      deno_json.specifier,
      exports_content
    );
    }
    let Some(name_no_at) = package.name.strip_prefix('@') else {
      bail!("Invalid package name, use '@<scope_name>/<package_name> format");
    };
    let Some((scope, name_no_scope)) = name_no_at.split_once('/') else {
      bail!("Invalid package name, use '@<scope_name>/<package_name> format");
    };
    let file_patterns = package.member_dir.to_publish_config()?.files;

    let tarball = deno_core::unsync::spawn_blocking({
      let diagnostics_collector = diagnostics_collector.clone();
      let unfurler = self.specifier_unfurler.clone();
      let cli_options = self.cli_options.clone();
      let source_cache = self.source_cache.clone();
      let config_path = config_path.clone();
      move || {
        let root_specifier =
          ModuleSpecifier::from_directory_path(&root_dir).unwrap();
        let mut publish_paths =
          paths::collect_publish_paths(paths::CollectPublishPathsOptions {
            root_dir: &root_dir,
            cli_options: &cli_options,
            diagnostics_collector: &diagnostics_collector,
            file_patterns,
            force_include_paths: vec![config_path],
          })?;
        collect_excluded_module_diagnostics(
          &root_specifier,
          &graph,
          &publish_paths,
          &diagnostics_collector,
        );

        if !has_license_file(publish_paths.iter().map(|p| &p.specifier)) {
          if let Some(license_path) =
            resolve_license_file(&root_dir, cli_options.workspace())
          {
            // force including the license file from the package or workspace root
            publish_paths.push(CollectedPublishPath {
              specifier: ModuleSpecifier::from_file_path(&license_path)
                .unwrap(),
              relative_path: "LICENSE".to_string(),
              maybe_content: Some(std::fs::read(&license_path).with_context(
                || format!("failed reading '{}'.", license_path.display()),
              )?),
              path: license_path,
            });
          } else {
            diagnostics_collector.push(PublishDiagnostic::MissingLicense {
              expected_path: root_dir.join("LICENSE"),
            });
          }
        }

        tar::create_gzipped_tarball(
          publish_paths,
          LazyGraphSourceParser::new(&source_cache, &graph),
          &diagnostics_collector,
          &unfurler,
        )
        .context("Failed to create a tarball")
      }
    })
    .await??;

    log::debug!("Tarball size ({}): {}", package.name, tarball.bytes.len());

    Ok(Rc::new(PreparedPublishPackage {
      scope: scope.to_string(),
      package: name_no_scope.to_string(),
      version: version.to_string(),
      tarball,
      exports: match &deno_json.json.exports {
        Some(Value::Object(exports)) => exports
          .into_iter()
          .map(|(k, v)| (k.to_string(), v.as_str().unwrap().to_string()))
          .collect(),
        Some(Value::String(exports)) => {
          let mut map = HashMap::new();
          map.insert(".".to_string(), exports.to_string());
          map
        }
        _ => HashMap::new(),
      },
      // the config file is always at the root of a publishing dir,
      // so getting the file name is always correct
      config: config_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string(),
    }))
  }
}

#[derive(Serialize)]
#[serde(tag = "permission")]
pub enum Permission<'s> {
  #[serde(rename = "package/publish", rename_all = "camelCase")]
  VersionPublish {
    scope: &'s str,
    package: &'s str,
    version: &'s str,
    tarball_hash: &'s str,
  },
}

async fn get_auth_headers(
  client: &HttpClient,
  registry_url: &Url,
  packages: &[Rc<PreparedPublishPackage>],
  auth_method: AuthMethod,
) -> Result<HashMap<(String, String, String), Rc<str>>, AnyError> {
  let permissions = packages
    .iter()
    .map(|package| Permission::VersionPublish {
      scope: &package.scope,
      package: &package.package,
      version: &package.version,
      tarball_hash: &package.tarball.hash,
    })
    .collect::<Vec<_>>();

  let mut authorizations = HashMap::with_capacity(packages.len());

  match auth_method {
    AuthMethod::Interactive => {
      let verifier = uuid::Uuid::new_v4().to_string();
      let challenge = BASE64_STANDARD.encode(sha2::Sha256::digest(&verifier));

      let response = client
        .post_json(
          format!("{}authorizations", registry_url).parse()?,
          &serde_json::json!({
            "challenge": challenge,
            "permissions": permissions,
          }),
        )?
        .send()
        .await
        .context("Failed to create interactive authorization")?;
      let auth =
        api::parse_response::<api::CreateAuthorizationResponse>(response)
          .await
          .context("Failed to create interactive authorization")?;

      let auth_url = format!("{}?code={}", auth.verification_url, auth.code);
      let pkgs_text = if packages.len() > 1 {
        format!("{} packages", packages.len())
      } else {
        format!("@{}/{}", packages[0].scope, packages[0].package)
      };
      log::warn!(
        "Visit {} to authorize publishing of {}",
        colors::cyan(&auth_url),
        pkgs_text,
      );

      ring_bell();
      log::info!("{}", colors::gray("Waiting..."));
      let _ = open::that_detached(&auth_url);

      let interval = std::time::Duration::from_secs(auth.poll_interval);

      loop {
        tokio::time::sleep(interval).await;
        let response = client
          .post_json(
            format!("{}authorizations/exchange", registry_url).parse()?,
            &serde_json::json!({
              "exchangeToken": auth.exchange_token,
              "verifier": verifier,
            }),
          )?
          .send()
          .await
          .context("Failed to exchange authorization")?;
        let res =
          api::parse_response::<api::ExchangeAuthorizationResponse>(response)
            .await;
        match res {
          Ok(res) => {
            log::info!(
              "{} {} {}",
              colors::green("Authorization successful."),
              colors::gray("Authenticated as"),
              colors::cyan(res.user.name)
            );
            let authorization: Rc<str> = format!("Bearer {}", res.token).into();
            for pkg in packages {
              authorizations.insert(
                (pkg.scope.clone(), pkg.package.clone(), pkg.version.clone()),
                authorization.clone(),
              );
            }
            break;
          }
          Err(err) => {
            if err.code == "authorizationPending" {
              continue;
            } else {
              return Err(err).context("Failed to exchange authorization");
            }
          }
        }
      }
    }
    AuthMethod::Token(token) => {
      let authorization: Rc<str> = format!("Bearer {}", token).into();
      for pkg in packages {
        authorizations.insert(
          (pkg.scope.clone(), pkg.package.clone(), pkg.version.clone()),
          authorization.clone(),
        );
      }
    }
    AuthMethod::Oidc(oidc_config) => {
      let mut chunked_packages = packages.chunks(16);
      for permissions in permissions.chunks(16) {
        let audience = json!({ "permissions": permissions }).to_string();
        let url = format!(
          "{}&audience={}",
          oidc_config.url,
          percent_encoding::percent_encode(
            audience.as_bytes(),
            percent_encoding::NON_ALPHANUMERIC
          )
        );

        let response = client
          .get(url.parse()?)?
          .header(
            http::header::AUTHORIZATION,
            format!("Bearer {}", oidc_config.token).parse()?,
          )
          .send()
          .await
          .context("Failed to get OIDC token")?;
        let status = response.status();
        let text = crate::http_util::body_to_string(response)
          .await
          .with_context(|| {
            format!("Failed to get OIDC token: status {}", status)
          })?;
        if !status.is_success() {
          bail!(
            "Failed to get OIDC token: status {}, response: '{}'",
            status,
            text
          );
        }
        let api::OidcTokenResponse { value } = serde_json::from_str(&text)
          .with_context(|| {
            format!(
              "Failed to parse OIDC token: '{}' (status {})",
              text, status
            )
          })?;

        let authorization: Rc<str> = format!("githuboidc {}", value).into();
        for pkg in chunked_packages.next().unwrap() {
          authorizations.insert(
            (pkg.scope.clone(), pkg.package.clone(), pkg.version.clone()),
            authorization.clone(),
          );
        }
      }
    }
  };

  Ok(authorizations)
}

/// Check if both `scope` and `package` already exist, if not return
/// a URL to the management panel to create them.
async fn check_if_scope_and_package_exist(
  client: &HttpClient,
  registry_api_url: &Url,
  registry_manage_url: &Url,
  scope: &str,
  package: &str,
) -> Result<Option<String>, AnyError> {
  let mut needs_scope = false;
  let mut needs_package = false;

  let response = api::get_scope(client, registry_api_url, scope).await?;
  if response.status() == 404 {
    needs_scope = true;
  }

  let response =
    api::get_package(client, registry_api_url, scope, package).await?;
  if response.status() == 404 {
    needs_package = true;
  }

  if needs_scope || needs_package {
    let create_url = format!(
      "{}new?scope={}&package={}&from=cli",
      registry_manage_url, scope, package
    );
    return Ok(Some(create_url));
  }

  Ok(None)
}

async fn ensure_scopes_and_packages_exist(
  client: &HttpClient,
  registry_api_url: &Url,
  registry_manage_url: &Url,
  packages: &[Rc<PreparedPublishPackage>],
) -> Result<(), AnyError> {
  if !std::io::stdin().is_terminal() {
    let mut missing_packages_lines = vec![];
    for package in packages {
      let maybe_create_package_url = check_if_scope_and_package_exist(
        client,
        registry_api_url,
        registry_manage_url,
        &package.scope,
        &package.package,
      )
      .await?;

      if let Some(create_package_url) = maybe_create_package_url {
        missing_packages_lines.push(format!(" - {}", create_package_url));
      }
    }

    if !missing_packages_lines.is_empty() {
      bail!(
        "Following packages don't exist, follow the links and create them:\n{}",
        missing_packages_lines.join("\n")
      );
    }
    return Ok(());
  }

  for package in packages {
    let maybe_create_package_url = check_if_scope_and_package_exist(
      client,
      registry_api_url,
      registry_manage_url,
      &package.scope,
      &package.package,
    )
    .await?;

    let Some(create_package_url) = maybe_create_package_url else {
      continue;
    };

    ring_bell();
    log::warn!(
      "'@{}/{}' doesn't exist yet. Visit {} to create the package",
      &package.scope,
      &package.package,
      colors::cyan_with_underline(&create_package_url)
    );
    log::warn!("{}", colors::gray("Waiting..."));
    let _ = open::that_detached(&create_package_url);

    let package_api_url = api::get_package_api_url(
      registry_api_url,
      &package.scope,
      &package.package,
    );

    loop {
      tokio::time::sleep(std::time::Duration::from_secs(3)).await;
      let response = client.get(package_api_url.parse()?)?.send().await?;
      if response.status() == 200 {
        let name = format!("@{}/{}", package.scope, package.package);
        log::info!("Package {} created", colors::green(name));
        break;
      }
    }
  }

  Ok(())
}

async fn perform_publish(
  http_client: &HttpClient,
  mut publish_order_graph: PublishOrderGraph,
  mut prepared_package_by_name: HashMap<String, Rc<PreparedPublishPackage>>,
  auth_method: AuthMethod,
  provenance: bool,
) -> Result<(), AnyError> {
  let registry_api_url = jsr_api_url();
  let registry_url = jsr_url();

  let packages = prepared_package_by_name
    .values()
    .cloned()
    .collect::<Vec<_>>();

  ensure_scopes_and_packages_exist(
    http_client,
    registry_api_url,
    registry_url,
    &packages,
  )
  .await?;

  let mut authorizations =
    get_auth_headers(http_client, registry_api_url, &packages, auth_method)
      .await?;

  assert_eq!(prepared_package_by_name.len(), authorizations.len());
  let mut futures: FuturesUnordered<LocalBoxFuture<Result<String, AnyError>>> =
    Default::default();
  loop {
    let next_batch = publish_order_graph.next();

    for package_name in next_batch {
      let package = prepared_package_by_name.remove(&package_name).unwrap();

      // todo(dsherret): output something that looks better than this even not in debug
      if log::log_enabled!(log::Level::Debug) {
        log::debug!("Publishing {}", package.display_name());
        for file in &package.tarball.files {
          log::debug!(
            "  Tarball file {} {}",
            human_size(file.size as f64),
            file.specifier
          );
        }
      }

      let authorization = authorizations
        .remove(&(
          package.scope.clone(),
          package.package.clone(),
          package.version.clone(),
        ))
        .unwrap();
      futures.push(
        async move {
          let display_name = package.display_name();
          publish_package(
            http_client,
            package,
            registry_api_url,
            registry_url,
            &authorization,
            provenance,
          )
          .await
          .with_context(|| format!("Failed to publish {}", display_name))?;
          Ok(package_name)
        }
        .boxed_local(),
      );
    }

    let Some(result) = futures.next().await else {
      // done, ensure no circular dependency
      publish_order_graph.ensure_no_pending()?;
      break;
    };

    let package_name = result?;
    publish_order_graph.finish_package(&package_name);
  }

  Ok(())
}

async fn publish_package(
  http_client: &HttpClient,
  package: Rc<PreparedPublishPackage>,
  registry_api_url: &Url,
  registry_url: &Url,
  authorization: &str,
  provenance: bool,
) -> Result<(), AnyError> {
  log::info!(
    "{} @{}/{}@{} ...",
    colors::intense_blue("Publishing"),
    package.scope,
    package.package,
    package.version
  );

  let url = format!(
    "{}scopes/{}/packages/{}/versions/{}?config=/{}",
    registry_api_url,
    package.scope,
    package.package,
    package.version,
    package.config
  );

  let body = http_body_util::Full::new(package.tarball.bytes.clone())
    .map_err(|never| match never {})
    .boxed();
  let response = http_client
    .post(url.parse()?, body)?
    .header(
      http::header::AUTHORIZATION,
      authorization.parse().map_err(http::Error::from)?,
    )
    .header(
      http::header::CONTENT_ENCODING,
      "gzip".parse().map_err(http::Error::from)?,
    )
    .send()
    .await?;

  let res = api::parse_response::<api::PublishingTask>(response).await;
  let mut task = match res {
    Ok(task) => task,
    Err(mut err) if err.code == "duplicateVersionPublish" => {
      let task = serde_json::from_value::<api::PublishingTask>(
        err.data.get_mut("task").unwrap().take(),
      )
      .unwrap();
      if task.status == "success" {
        log::info!(
          "{} @{}/{}@{}",
          colors::yellow("Warning: Skipping, already published"),
          package.scope,
          package.package,
          package.version
        );
        return Ok(());
      }
      log::info!(
        "{} @{}/{}@{}",
        colors::yellow("Already uploaded, waiting for publishing"),
        package.scope,
        package.package,
        package.version
      );
      task
    }
    Err(err) => {
      return Err(err).with_context(|| {
        format!(
          "Failed to publish @{}/{} at {}",
          package.scope, package.package, package.version
        )
      })
    }
  };

  let interval = std::time::Duration::from_secs(2);
  while task.status != "success" && task.status != "failure" {
    tokio::time::sleep(interval).await;
    let resp = http_client
      .get(format!("{}publish_status/{}", registry_api_url, task.id).parse()?)?
      .send()
      .await
      .with_context(|| {
        format!(
          "Failed to get publishing status for @{}/{} at {}",
          package.scope, package.package, package.version
        )
      })?;
    task = api::parse_response::<api::PublishingTask>(resp)
      .await
      .with_context(|| {
        format!(
          "Failed to get publishing status for @{}/{} at {}",
          package.scope, package.package, package.version
        )
      })?;
  }

  if let Some(error) = task.error {
    bail!(
      "{} @{}/{} at {}: {}",
      colors::red("Failed to publish"),
      package.scope,
      package.package,
      package.version,
      error.message
    );
  }

  log::info!(
    "{} @{}/{}@{}",
    colors::green("Successfully published"),
    package.scope,
    package.package,
    package.version
  );

  let enable_provenance = std::env::var("DISABLE_JSR_PROVENANCE").is_err()
    && (auth::is_gha() && auth::gha_oidc_token().is_some() && provenance);

  // Enable provenance by default on Github actions with OIDC token
  if enable_provenance {
    // Get the version manifest from the registry
    let meta_url = jsr_url().join(&format!(
      "@{}/{}/{}_meta.json",
      package.scope, package.package, package.version
    ))?;

    let resp = http_client.get(meta_url)?.send().await?;
    let meta_bytes = resp.collect().await?.to_bytes();

    if std::env::var("DISABLE_JSR_MANIFEST_VERIFICATION_FOR_TESTING").is_err() {
      verify_version_manifest(&meta_bytes, &package)?;
    }

    let subject = provenance::Subject {
      name: format!(
        "pkg:jsr/@{}/{}@{}",
        package.scope, package.package, package.version
      ),
      digest: provenance::SubjectDigest {
        sha256: faster_hex::hex_string(&sha2::Sha256::digest(&meta_bytes)),
      },
    };
    let bundle = provenance::generate_provenance(http_client, subject).await?;

    let tlog_entry = &bundle.verification_material.tlog_entries[0];
    log::info!("{}",
      colors::green(format!(
        "Provenance transparency log available at https://search.sigstore.dev/?logIndex={}",
        tlog_entry.log_index
      ))
     );

    // Submit bundle to JSR
    let provenance_url = format!(
      "{}scopes/{}/packages/{}/versions/{}/provenance",
      registry_api_url, package.scope, package.package, package.version
    );
    http_client
      .post_json(provenance_url.parse()?, &json!({ "bundle": bundle }))?
      .header(http::header::AUTHORIZATION, authorization.parse()?)
      .send()
      .await?;
  }

  log::info!(
    "{}",
    colors::gray(format!(
      "Visit {}@{}/{}@{} for details",
      registry_url, package.scope, package.package, package.version
    ))
  );
  Ok(())
}

fn collect_excluded_module_diagnostics(
  root: &ModuleSpecifier,
  graph: &deno_graph::ModuleGraph,
  publish_paths: &[CollectedPublishPath],
  diagnostics_collector: &PublishDiagnosticsCollector,
) {
  let publish_specifiers = publish_paths
    .iter()
    .map(|path| &path.specifier)
    .collect::<HashSet<_>>();
  let graph_specifiers = graph
    .modules()
    .filter_map(|m| match m {
      deno_graph::Module::Js(_) | deno_graph::Module::Json(_) => {
        Some(m.specifier())
      }
      deno_graph::Module::Npm(_)
      | deno_graph::Module::Node(_)
      | deno_graph::Module::External(_) => None,
    })
    .filter(|s| s.as_str().starts_with(root.as_str()));
  for specifier in graph_specifiers {
    if !publish_specifiers.contains(specifier) {
      diagnostics_collector.push(PublishDiagnostic::ExcludedModule {
        specifier: specifier.clone(),
      });
    }
  }
}

#[derive(Deserialize)]
struct ManifestEntry {
  checksum: String,
}

#[derive(Deserialize)]
struct VersionManifest {
  manifest: HashMap<String, ManifestEntry>,
  exports: HashMap<String, String>,
}

fn verify_version_manifest(
  meta_bytes: &[u8],
  package: &PreparedPublishPackage,
) -> Result<(), AnyError> {
  let manifest = serde_json::from_slice::<VersionManifest>(meta_bytes)?;
  // Check that nothing was removed from the manifest.
  if manifest.manifest.len() != package.tarball.files.len() {
    bail!(
      "Mismatch in the number of files in the manifest: expected {}, got {}",
      package.tarball.files.len(),
      manifest.manifest.len()
    );
  }

  for (path, entry) in manifest.manifest {
    // Verify each path with the files in the tarball.
    let file = package
      .tarball
      .files
      .iter()
      .find(|f| f.path_str == path.as_str());

    if let Some(file) = file {
      if file.hash != entry.checksum {
        bail!(
          "Checksum mismatch for {}: expected {}, got {}",
          path,
          entry.checksum,
          file.hash
        );
      }
    } else {
      bail!("File {} not found in the tarball", path);
    }
  }

  for (specifier, expected) in &manifest.exports {
    let actual = package.exports.get(specifier).ok_or_else(|| {
      deno_core::anyhow::anyhow!(
        "Export {} not found in the package",
        specifier
      )
    })?;
    if actual != expected {
      bail!(
        "Export {} mismatch: expected {}, got {}",
        specifier,
        expected,
        actual
      );
    }
  }

  Ok(())
}

async fn check_if_git_repo_dirty(cwd: &Path) -> Option<String> {
  let bin_name = if cfg!(windows) { "git.exe" } else { "git" };

  //  Check if git exists
  let git_exists = Command::new(bin_name)
    .arg("--version")
    .stderr(Stdio::null())
    .stdout(Stdio::null())
    .status()
    .await
    .map_or(false, |status| status.success());

  if !git_exists {
    return None; // Git is not installed
  }

  // Check if there are uncommitted changes
  let output = Command::new(bin_name)
    .current_dir(cwd)
    .args(["status", "--porcelain"])
    .output()
    .await
    .expect("Failed to execute command");

  let output_str = String::from_utf8_lossy(&output.stdout);
  let text = output_str.trim();
  if text.is_empty() {
    None
  } else {
    Some(text.to_string())
  }
}

static SUPPORTED_LICENSE_FILE_NAMES: [&str; 6] = [
  "LICENSE",
  "LICENSE.md",
  "LICENSE.txt",
  "LICENCE",
  "LICENCE.md",
  "LICENCE.txt",
];

fn resolve_license_file(
  pkg_root_dir: &Path,
  workspace: &Workspace,
) -> Option<PathBuf> {
  let workspace_root_dir = workspace.root_dir_path();
  let mut dirs = Vec::with_capacity(2);
  dirs.push(pkg_root_dir);
  if workspace_root_dir != pkg_root_dir {
    dirs.push(&workspace_root_dir);
  }
  for dir in dirs {
    for file_name in &SUPPORTED_LICENSE_FILE_NAMES {
      let file_path = dir.join(file_name);
      if file_path.exists() {
        return Some(file_path);
      }
    }
  }
  None
}

fn has_license_file<'a>(
  mut specifiers: impl Iterator<Item = &'a ModuleSpecifier>,
) -> bool {
  let supported_license_files = SUPPORTED_LICENSE_FILE_NAMES
    .iter()
    .map(|s| s.to_lowercase())
    .collect::<HashSet<_>>();
  specifiers.any(|specifier| {
    specifier
      .path()
      .rsplit_once('/')
      .map(|(_, file)| {
        supported_license_files.contains(file.to_lowercase().as_str())
      })
      .unwrap_or(false)
  })
}

#[allow(clippy::print_stderr)]
fn ring_bell() {
  // ASCII code for the bell character.
  eprint!("\x07");
}

#[cfg(test)]
mod tests {
  use deno_ast::ModuleSpecifier;

  use crate::tools::registry::has_license_file;

  use super::tar::PublishableTarball;
  use super::tar::PublishableTarballFile;
  use super::verify_version_manifest;
  use std::collections::HashMap;

  #[test]
  fn test_verify_version_manifest() {
    let meta = r#"{
      "manifest": {
        "mod.ts": {
          "checksum": "abc123"
        }
      },
      "exports": {}
    }"#;

    let meta_bytes = meta.as_bytes();
    let package = super::PreparedPublishPackage {
      scope: "test".to_string(),
      package: "test".to_string(),
      version: "1.0.0".to_string(),
      tarball: PublishableTarball {
        bytes: vec![].into(),
        hash: "abc123".to_string(),
        files: vec![PublishableTarballFile {
          specifier: "file://mod.ts".try_into().unwrap(),
          path_str: "mod.ts".to_string(),
          hash: "abc123".to_string(),
          size: 0,
        }],
      },
      config: "deno.json".to_string(),
      exports: HashMap::new(),
    };

    assert!(verify_version_manifest(meta_bytes, &package).is_ok());
  }

  #[test]
  fn test_verify_version_manifest_missing() {
    let meta = r#"{
      "manifest": {
        "mod.ts": {},
      },
      "exports": {}
    }"#;

    let meta_bytes = meta.as_bytes();
    let package = super::PreparedPublishPackage {
      scope: "test".to_string(),
      package: "test".to_string(),
      version: "1.0.0".to_string(),
      tarball: PublishableTarball {
        bytes: vec![].into(),
        hash: "abc123".to_string(),
        files: vec![PublishableTarballFile {
          specifier: "file://mod.ts".try_into().unwrap(),
          path_str: "mod.ts".to_string(),
          hash: "abc123".to_string(),
          size: 0,
        }],
      },
      config: "deno.json".to_string(),
      exports: HashMap::new(),
    };

    assert!(verify_version_manifest(meta_bytes, &package).is_err());
  }

  #[test]
  fn test_verify_version_manifest_invalid_hash() {
    let meta = r#"{
      "manifest": {
        "mod.ts": {
          "checksum": "lol123"
        },
        "exports": {}
      }
    }"#;

    let meta_bytes = meta.as_bytes();
    let package = super::PreparedPublishPackage {
      scope: "test".to_string(),
      package: "test".to_string(),
      version: "1.0.0".to_string(),
      tarball: PublishableTarball {
        bytes: vec![].into(),
        hash: "abc123".to_string(),
        files: vec![PublishableTarballFile {
          specifier: "file://mod.ts".try_into().unwrap(),
          path_str: "mod.ts".to_string(),
          hash: "abc123".to_string(),
          size: 0,
        }],
      },
      config: "deno.json".to_string(),
      exports: HashMap::new(),
    };

    assert!(verify_version_manifest(meta_bytes, &package).is_err());
  }

  #[test]
  fn test_has_license_files() {
    fn has_license_file_str(expected: &[&str]) -> bool {
      let specifiers = expected
        .iter()
        .map(|s| ModuleSpecifier::parse(s).unwrap())
        .collect::<Vec<_>>();
      has_license_file(specifiers.iter())
    }

    assert!(has_license_file_str(&["file:///LICENSE"]));
    assert!(has_license_file_str(&["file:///license"]));
    assert!(has_license_file_str(&["file:///LICENSE.txt"]));
    assert!(has_license_file_str(&["file:///LICENSE.md"]));
    assert!(has_license_file_str(&["file:///LICENCE"]));
    assert!(has_license_file_str(&["file:///LICENCE.txt"]));
    assert!(has_license_file_str(&["file:///LICENCE.md"]));
    assert!(has_license_file_str(&[
      "file:///other",
      "file:///test/LICENCE.md"
    ]),);
    assert!(!has_license_file_str(&[
      "file:///other",
      "file:///test/tLICENSE"
    ]),);
  }
}
