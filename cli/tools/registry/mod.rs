// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::rc::Rc;
use std::sync::Arc;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use deno_config::ConfigFile;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::unsync::JoinSet;
use deno_runtime::deno_fetch::reqwest;
use deno_terminal::colors;
use import_map::ImportMap;
use lsp_types::Url;
use serde::Serialize;
use sha2::Digest;

use crate::args::deno_registry_api_url;
use crate::args::deno_registry_url;
use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::PublishFlags;
use crate::cache::LazyGraphSourceParser;
use crate::cache::ParsedSourceCache;
use crate::factory::CliFactory;
use crate::graph_util::ModuleGraphBuilder;
use crate::http_util::HttpClient;
use crate::tools::check::CheckOptions;
use crate::tools::registry::diagnostics::PublishDiagnosticsCollector;
use crate::tools::registry::graph::collect_fast_check_type_graph_diagnostics;
use crate::tools::registry::graph::collect_invalid_external_imports;
use crate::tools::registry::graph::get_workspace_member_roots;
use crate::tools::registry::graph::resolve_config_file_roots_from_exports;
use crate::tools::registry::graph::MemberRoots;
use crate::util::display::human_size;
use crate::util::import_map::ImportMapUnfurler;

mod api;
mod auth;
mod diagnostics;
mod graph;
mod paths;
mod publish_order;
mod tar;

use auth::get_auth_method;
use auth::AuthMethod;
use publish_order::PublishOrderGraph;

use super::check::TypeChecker;

use self::tar::PublishableTarball;

fn ring_bell() {
  // ASCII code for the bell character.
  print!("\x07");
}

struct PreparedPublishPackage {
  scope: String,
  package: String,
  version: String,
  tarball: PublishableTarball,
}

impl PreparedPublishPackage {
  pub fn display_name(&self) -> String {
    format!("@{}/{}@{}", self.scope, self.package, self.version)
  }
}

static SUGGESTED_ENTRYPOINTS: [&str; 4] =
  ["mod.ts", "mod.js", "index.ts", "index.js"];

fn get_deno_json_package_name(
  deno_json: &ConfigFile,
) -> Result<String, AnyError> {
  match deno_json.json.name.clone() {
    Some(name) => Ok(name),
    None => bail!("{} is missing 'name' field", deno_json.specifier),
  }
}

async fn prepare_publish(
  deno_json: &ConfigFile,
  source_cache: Arc<ParsedSourceCache>,
  graph: Arc<deno_graph::ModuleGraph>,
  import_map: Arc<ImportMap>,
  diagnostics_collector: &PublishDiagnosticsCollector,
) -> Result<Rc<PreparedPublishPackage>, AnyError> {
  let config_path = deno_json.specifier.to_file_path().unwrap();
  let dir_path = config_path.parent().unwrap().to_path_buf();
  let Some(version) = deno_json.json.version.clone() else {
    bail!("{} is missing 'version' field", deno_json.specifier);
  };
  let name = get_deno_json_package_name(deno_json)?;
  if deno_json.json.exports.is_none() {
    let mut suggested_entrypoint = None;

    for entrypoint in SUGGESTED_ENTRYPOINTS {
      if dir_path.join(entrypoint).exists() {
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
      name,
      version,
      suggested_entrypoint.unwrap_or("<path_to_entrypoint>")
    );

    bail!(
      "You did not specify an entrypoint to \"{}\" package in {}. Add `exports` mapping in the configuration file, eg:\n{}",
      name,
      deno_json.specifier,
      exports_content
    );
  }
  let Some(name) = name.strip_prefix('@') else {
    bail!("Invalid package name, use '@<scope_name>/<package_name> format");
  };
  let Some((scope, package_name)) = name.split_once('/') else {
    bail!("Invalid package name, use '@<scope_name>/<package_name> format");
  };
  let file_patterns = deno_json.to_publish_config()?.map(|c| c.files);

  let diagnostics_collector = diagnostics_collector.clone();
  let tarball = deno_core::unsync::spawn_blocking(move || {
    let unfurler = ImportMapUnfurler::new(&import_map);
    tar::create_gzipped_tarball(
      &dir_path,
      LazyGraphSourceParser::new(&source_cache, &graph),
      &diagnostics_collector,
      &unfurler,
      file_patterns,
    )
    .context("Failed to create a tarball")
  })
  .await??;

  log::debug!("Tarball size ({}): {}", name, tarball.bytes.len());

  Ok(Rc::new(PreparedPublishPackage {
    scope: scope.to_string(),
    package: package_name.to_string(),
    version: version.to_string(),
    tarball,
  }))
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
  client: &reqwest::Client,
  registry_url: String,
  packages: Vec<Rc<PreparedPublishPackage>>,
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
        .post(format!("{}authorizations", registry_url))
        .json(&serde_json::json!({
          "challenge": challenge,
          "permissions": permissions,
        }))
        .send()
        .await
        .context("Failed to create interactive authorization")?;
      let auth =
        api::parse_response::<api::CreateAuthorizationResponse>(response)
          .await
          .context("Failed to create interactive authorization")?;

      let auth_url = format!("{}?code={}", auth.verification_url, auth.code);
      print!(
        "Visit {} to authorize publishing of",
        colors::cyan(&auth_url)
      );
      if packages.len() > 1 {
        println!(" {} packages", packages.len());
      } else {
        println!(" @{}/{}", packages[0].scope, packages[0].package);
      }

      ring_bell();
      println!("{}", colors::gray("Waiting..."));
      let _ = open::that_detached(&auth_url);

      let interval = std::time::Duration::from_secs(auth.poll_interval);

      loop {
        tokio::time::sleep(interval).await;
        let response = client
          .post(format!("{}authorizations/exchange", registry_url))
          .json(&serde_json::json!({
            "exchangeToken": auth.exchange_token,
            "verifier": verifier,
          }))
          .send()
          .await
          .context("Failed to exchange authorization")?;
        let res =
          api::parse_response::<api::ExchangeAuthorizationResponse>(response)
            .await;
        match res {
          Ok(res) => {
            println!(
              "{} {} {}",
              colors::green("Authorization successful."),
              colors::gray("Authenticated as"),
              colors::cyan(res.user.name)
            );
            let authorization: Rc<str> = format!("Bearer {}", res.token).into();
            for pkg in &packages {
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
      for pkg in &packages {
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
          .get(url)
          .bearer_auth(&oidc_config.token)
          .send()
          .await
          .context("Failed to get OIDC token")?;
        let status = response.status();
        let text = response.text().await.with_context(|| {
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
  client: &reqwest::Client,
  registry_api_url: &str,
  registry_manage_url: &str,
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
  client: &reqwest::Client,
  registry_api_url: String,
  registry_manage_url: String,
  packages: Vec<Rc<PreparedPublishPackage>>,
) -> Result<(), AnyError> {
  if !std::io::stdin().is_terminal() {
    let mut missing_packages_lines = vec![];
    for package in packages {
      let maybe_create_package_url = check_if_scope_and_package_exist(
        client,
        &registry_api_url,
        &registry_manage_url,
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
      &registry_api_url,
      &registry_manage_url,
      &package.scope,
      &package.package,
    )
    .await?;

    let Some(create_package_url) = maybe_create_package_url else {
      continue;
    };

    ring_bell();
    println!(
      "'@{}/{}' doesn't exist yet. Visit {} to create the package",
      &package.scope,
      &package.package,
      colors::cyan_with_underline(&create_package_url)
    );
    println!("{}", colors::gray("Waiting..."));
    let _ = open::that_detached(&create_package_url);

    let package_api_url = api::get_package_api_url(
      &registry_api_url,
      &package.scope,
      &package.package,
    );

    loop {
      tokio::time::sleep(std::time::Duration::from_secs(3)).await;
      let response = client.get(&package_api_url).send().await?;
      if response.status() == 200 {
        let name = format!("@{}/{}", package.scope, package.package);
        println!("Package {} created", colors::green(name));
        break;
      }
    }
  }

  Ok(())
}

async fn perform_publish(
  http_client: &Arc<HttpClient>,
  mut publish_order_graph: PublishOrderGraph,
  mut prepared_package_by_name: HashMap<String, Rc<PreparedPublishPackage>>,
  auth_method: AuthMethod,
) -> Result<(), AnyError> {
  let client = http_client.client()?;
  let registry_api_url = deno_registry_api_url().to_string();
  let registry_url = deno_registry_url().to_string();

  let packages = prepared_package_by_name
    .values()
    .cloned()
    .collect::<Vec<_>>();

  ensure_scopes_and_packages_exist(
    client,
    registry_api_url.clone(),
    registry_url.clone(),
    packages.clone(),
  )
  .await?;

  let mut authorizations =
    get_auth_headers(client, registry_api_url.clone(), packages, auth_method)
      .await?;

  assert_eq!(prepared_package_by_name.len(), authorizations.len());
  let mut futures: JoinSet<Result<String, AnyError>> = JoinSet::default();
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
      let registry_api_url = registry_api_url.clone();
      let registry_url = registry_url.clone();
      let http_client = http_client.clone();
      futures.spawn(async move {
        let display_name = package.display_name();
        publish_package(
          &http_client,
          package,
          &registry_api_url,
          &registry_url,
          &authorization,
        )
        .await
        .with_context(|| format!("Failed to publish {}", display_name))?;
        Ok(package_name)
      });
    }

    let Some(result) = futures.join_next().await else {
      // done, ensure no circular dependency
      publish_order_graph.ensure_no_pending()?;
      break;
    };

    let package_name = result??;
    publish_order_graph.finish_package(&package_name);
  }

  Ok(())
}

async fn publish_package(
  http_client: &HttpClient,
  package: Rc<PreparedPublishPackage>,
  registry_api_url: &str,
  registry_url: &str,
  authorization: &str,
) -> Result<(), AnyError> {
  let client = http_client.client()?;
  println!(
    "{} @{}/{}@{} ...",
    colors::intense_blue("Publishing"),
    package.scope,
    package.package,
    package.version
  );

  let url = format!(
    "{}scopes/{}/packages/{}/versions/{}",
    registry_api_url, package.scope, package.package, package.version
  );

  let response = client
    .post(url)
    .header(reqwest::header::AUTHORIZATION, authorization)
    .header(reqwest::header::CONTENT_ENCODING, "gzip")
    .body(package.tarball.bytes.clone())
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
        println!(
          "{} @{}/{}@{}",
          colors::green("Skipping, already published"),
          package.scope,
          package.package,
          package.version
        );
        return Ok(());
      }
      println!(
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
    let resp = client
      .get(format!("{}publish_status/{}", registry_api_url, task.id))
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

  println!(
    "{} @{}/{}@{}",
    colors::green("Successfully published"),
    package.scope,
    package.package,
    package.version
  );
  println!(
    "{}",
    colors::gray(format!(
      "Visit {}@{}/{}@{} for details",
      registry_url, package.scope, package.package, package.version
    ))
  );
  Ok(())
}

struct PreparePackagesData {
  publish_order_graph: PublishOrderGraph,
  package_by_name: HashMap<String, Rc<PreparedPublishPackage>>,
}

async fn prepare_packages_for_publishing(
  cli_factory: &CliFactory,
  no_zap: bool,
  diagnostics_collector: &PublishDiagnosticsCollector,
  deno_json: ConfigFile,
  import_map: Arc<ImportMap>,
) -> Result<PreparePackagesData, AnyError> {
  let maybe_workspace_config = deno_json.to_workspace_config()?;
  let module_graph_builder = cli_factory.module_graph_builder().await?.as_ref();
  let source_cache = cli_factory.parsed_source_cache();
  let type_checker = cli_factory.type_checker().await?;
  let cli_options = cli_factory.cli_options();

  let Some(workspace_config) = maybe_workspace_config else {
    let roots = resolve_config_file_roots_from_exports(&deno_json)?;
    let graph = build_and_check_graph_for_publish(
      module_graph_builder,
      type_checker,
      cli_options,
      no_zap,
      diagnostics_collector,
      &[MemberRoots {
        name: get_deno_json_package_name(&deno_json)?,
        dir_url: deno_json.specifier.join("./").unwrap().clone(),
        exports: roots,
      }],
    )
    .await?;
    let package = prepare_publish(
      &deno_json,
      source_cache.clone(),
      graph,
      import_map,
      diagnostics_collector,
    )
    .await?;
    let package_name = format!("@{}/{}", package.scope, package.package);
    let publish_order_graph =
      PublishOrderGraph::new_single(package_name.clone());
    let package_by_name = HashMap::from([(package_name, package)]);
    return Ok(PreparePackagesData {
      publish_order_graph,
      package_by_name,
    });
  };

  println!("Publishing a workspace...");
  // create the module graph
  let roots = get_workspace_member_roots(&workspace_config)?;
  let graph = build_and_check_graph_for_publish(
    module_graph_builder,
    type_checker,
    cli_options,
    no_zap,
    diagnostics_collector,
    &roots,
  )
  .await?;

  let mut package_by_name =
    HashMap::with_capacity(workspace_config.members.len());
  let publish_order_graph =
    publish_order::build_publish_order_graph(&graph, &roots)?;

  let results = workspace_config
    .members
    .iter()
    .cloned()
    .map(|member| {
      let import_map = import_map.clone();
      let graph = graph.clone();
      async move {
        let package = prepare_publish(
          &member.config_file,
          source_cache.clone(),
          graph,
          import_map,
          diagnostics_collector,
        )
        .await
        .with_context(|| {
          format!("Failed preparing '{}'.", member.package_name)
        })?;
        Ok::<_, AnyError>((member.package_name, package))
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
  module_graph_builder: &ModuleGraphBuilder,
  type_checker: &TypeChecker,
  cli_options: &CliOptions,
  no_zap: bool,
  diagnostics_collector: &PublishDiagnosticsCollector,
  packages: &[MemberRoots],
) -> Result<Arc<deno_graph::ModuleGraph>, deno_core::anyhow::Error> {
  let graph = Arc::new(
    module_graph_builder
      .create_graph_with_options(crate::graph_util::CreateGraphOptions {
        // All because we're going to use this same graph to determine the publish order later
        graph_kind: deno_graph::GraphKind::All,
        roots: packages
          .iter()
          .flat_map(|r| r.exports.iter())
          .cloned()
          .collect(),
        workspace_fast_check: true,
        loader: None,
      })
      .await?,
  );
  graph.valid()?;

  collect_invalid_external_imports(&graph, diagnostics_collector);

  let mut has_fast_check_diagnostics = false;
  if !no_zap {
    log::info!("Checking fast check type graph for errors...");
    has_fast_check_diagnostics = collect_fast_check_type_graph_diagnostics(
      &graph,
      packages,
      diagnostics_collector,
    );
  }

  if !has_fast_check_diagnostics {
    log::info!("Ensuring type checks...");
    let diagnostics = type_checker
      .check_diagnostics(
        graph.clone(),
        CheckOptions {
          lib: cli_options.ts_type_lib_window(),
          log_ignored_options: false,
          reload: cli_options.reload_flag(),
        },
      )
      .await?;
    if !diagnostics.is_empty() {
      bail!(
        concat!(
          "{:#}\n\n",
          "You may have discovered a bug in Deno's fast check implementation. ",
          "Fast check is still early days and we would appreciate if you log a ",
          "bug if you believe this is one: https://github.com/denoland/deno/issues/"
        ),
        diagnostics
      );
    }
  }
  Ok(graph)
}

pub async fn publish(
  flags: Flags,
  publish_flags: PublishFlags,
) -> Result<(), AnyError> {
  let cli_factory = CliFactory::from_flags(flags).await?;

  let auth_method = get_auth_method(publish_flags.token)?;

  let import_map = cli_factory
    .maybe_import_map()
    .await?
    .clone()
    .unwrap_or_else(|| {
      Arc::new(ImportMap::new(Url::parse("file:///dev/null").unwrap()))
    });

  let directory_path = cli_factory.cli_options().initial_cwd();

  let cli_options = cli_factory.cli_options();
  let Some(config_file) = cli_options.maybe_config_file() else {
    bail!(
      "Couldn't find a deno.json or a deno.jsonc configuration file in {}.",
      directory_path.display()
    );
  };

  let diagnostics_collector = PublishDiagnosticsCollector::default();

  let prepared_data = prepare_packages_for_publishing(
    &cli_factory,
    publish_flags.no_zap,
    &diagnostics_collector,
    config_file.clone(),
    import_map,
  )
  .await?;

  diagnostics_collector.print_and_error()?;

  if prepared_data.package_by_name.is_empty() {
    bail!("No packages to publish");
  }

  if publish_flags.dry_run {
    log::warn!(
      "{} Aborting due to --dry-run",
      crate::colors::yellow("Warning")
    );
    return Ok(());
  }

  perform_publish(
    cli_factory.http_client(),
    prepared_data.publish_order_graph,
    prepared_data.package_by_name,
    auth_method,
  )
  .await
}
