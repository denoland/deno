// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::fmt::Write;
use std::io::IsTerminal;
use std::rc::Rc;
use std::sync::Arc;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use deno_config::ConfigFile;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::unsync::JoinHandle;
use deno_core::unsync::JoinSet;
use deno_runtime::colors;
use deno_runtime::deno_fetch::reqwest;
use import_map::ImportMap;
use lsp_types::Url;
use serde::Serialize;
use sha2::Digest;

use crate::args::deno_registry_api_url;
use crate::args::deno_registry_url;
use crate::args::Flags;
use crate::args::PublishFlags;
use crate::factory::CliFactory;
use crate::http_util::HttpClient;
use crate::util::import_map::ImportMapUnfurler;

mod api;
mod auth;
mod publish_order;
mod tar;

use auth::get_auth_method;
use auth::AuthMethod;
use publish_order::PublishOrderGraph;

fn ring_bell() {
  // ASCII code for the bell character.
  print!("\x07");
}

struct PreparedPublishPackage {
  scope: String,
  package: String,
  version: String,
  tarball_hash: String,
  tarball: Bytes,
  diagnostics: Vec<String>,
}

static SUGGESTED_ENTRYPOINTS: [&str; 4] =
  ["mod.ts", "mod.js", "index.ts", "index.js"];

async fn prepare_publish(
  deno_json: &ConfigFile,
  import_map: Arc<ImportMap>,
) -> Result<Rc<PreparedPublishPackage>, AnyError> {
  let config_path = deno_json.specifier.to_file_path().unwrap();
  let dir_path = config_path.parent().unwrap().to_path_buf();
  let Some(version) = deno_json.json.version.clone() else {
    bail!("{} is missing 'version' field", deno_json.specifier);
  };
  let Some(name) = deno_json.json.name.clone() else {
    bail!("{} is missing 'name' field", deno_json.specifier);
  };
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

  let (tarball, diagnostics) = deno_core::unsync::spawn_blocking(move || {
    let unfurler = ImportMapUnfurler::new(&import_map);
    tar::create_gzipped_tarball(&dir_path, unfurler)
      .context("Failed to create a tarball")
  })
  .await??;

  let tarball_hash_bytes: Vec<u8> =
    sha2::Sha256::digest(&tarball).iter().cloned().collect();
  let mut tarball_hash = "sha256-".to_string();
  for byte in tarball_hash_bytes {
    write!(&mut tarball_hash, "{:02x}", byte).unwrap();
  }

  Ok(Rc::new(PreparedPublishPackage {
    scope: scope.to_string(),
    package: package_name.to_string(),
    version: version.to_string(),
    tarball_hash,
    tarball,
    diagnostics,
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

/// Prints diagnostics like so:
/// ```
///
/// Warning
/// ├╌ Dynamic import was not analyzable...
/// ├╌╌ at file:///dev/foo/bar/foo.ts:4:5
/// |
/// ├╌ Dynamic import was not analyzable...
/// ├╌╌ at file:///dev/foo/bar/foo.ts:4:5
/// |
/// ├╌ Dynamic import was not analyzable...
/// └╌╌ at file:///dev/foo/bar/foo.ts:4:5
///
/// ```
fn print_diagnostics(diagnostics: Vec<String>) {
  if !diagnostics.is_empty() {
    let len = diagnostics.len();
    log::warn!("");
    log::warn!("{}", crate::colors::yellow("Warning"));
    for (i, diagnostic) in diagnostics.iter().enumerate() {
      let last_diagnostic = i == len - 1;
      let lines = diagnostic.split('\n').collect::<Vec<_>>();
      let lines_len = lines.len();
      if i != 0 {
        log::warn!("|");
      }
      for (j, line) in lines.iter().enumerate() {
        let last_line = j == lines_len - 1;
        if j == 0 {
          log::warn!("├╌ {}", line);
        } else if last_line && last_diagnostic {
          log::warn!("└╌╌ {}", line);
        } else {
          log::warn!("├╌╌ {}", line);
        }
      }
    }
    log::warn!("");
  }
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
      tarball_hash: &package.tarball_hash,
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

      print!(
        "Visit {} to authorize publishing of",
        colors::cyan(format!("{}?code={}", auth.verification_url, auth.code))
      );
      if packages.len() > 1 {
        println!(" {} packages", packages.len());
      } else {
        println!(" @{}/{}", packages[0].scope, packages[0].package);
      }

      ring_bell();
      println!("{}", colors::gray("Waiting..."));

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
      colors::cyan_with_underline(create_package_url)
    );
    println!("{}", colors::gray("Waiting..."));

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
  let diagnostics = packages
    .iter()
    .flat_map(|p| p.diagnostics.clone())
    .collect::<Vec<_>>();
  print_diagnostics(diagnostics);

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
        let display_name =
          format!("@{}/{}@{}", package.scope, package.package, package.version);
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
    .body(package.tarball.clone())
    .send()
    .await?;

  let res = api::parse_response::<api::PublishingTask>(response).await;
  let mut task = match res {
    Ok(task) => task,
    Err(err) if err.code == "duplicateVersionPublish" => {
      println!(
        "{} @{}/{}@{}",
        colors::yellow("Skipping, already published"),
        package.scope,
        package.package,
        package.version
      );
      return Ok(());
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

async fn prepare_packages_for_publishing(
  cli_factory: &CliFactory,
  deno_json: ConfigFile,
  import_map: Arc<ImportMap>,
) -> Result<
  (
    PublishOrderGraph,
    HashMap<String, Rc<PreparedPublishPackage>>,
  ),
  AnyError,
> {
  let maybe_workspace_config = deno_json.to_workspace_config()?;

  let Some(workspace_config) = maybe_workspace_config else {
    let mut prepared_package_by_name = HashMap::with_capacity(1);
    let package = prepare_publish(&deno_json, import_map).await?;
    let package_name = package.package.clone();
    let publish_order_graph =
      PublishOrderGraph::new_single(package_name.clone());
    prepared_package_by_name.insert(package_name, package);
    return Ok((publish_order_graph, prepared_package_by_name));
  };

  println!("Publishing a workspace...");
  let mut prepared_package_by_name =
    HashMap::with_capacity(workspace_config.members.len());
  let publish_order_graph = publish_order::build_publish_graph(
    &workspace_config,
    cli_factory.module_graph_builder().await?.as_ref(),
  )
  .await?;

  let results =
    workspace_config
      .members
      .iter()
      .cloned()
      .map(|member| {
        let import_map = import_map.clone();
        deno_core::unsync::spawn(async move {
          let package = prepare_publish(&member.config_file, import_map)
            .await
            .with_context(|| {
              format!("Failed preparing '{}'.", member.package_name)
            })?;
          Ok((member.package_name, package))
        })
      })
      .collect::<Vec<
        JoinHandle<Result<(String, Rc<PreparedPublishPackage>), AnyError>>,
      >>();
  let results = deno_core::futures::future::join_all(results).await;
  for result in results {
    let (package_name, package) = result??;
    prepared_package_by_name.insert(package_name, package);
  }
  Ok((publish_order_graph, prepared_package_by_name))
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

  let initial_cwd =
    std::env::current_dir().with_context(|| "Failed getting cwd.")?;

  let directory_path = initial_cwd.join(publish_flags.directory);
  // TODO: doesn't handle jsonc
  let deno_json_path = directory_path.join("deno.json");
  let deno_json = ConfigFile::read(&deno_json_path).with_context(|| {
    format!(
      "Failed to read deno.json file at {}",
      deno_json_path.display()
    )
  })?;

  let (publish_order_graph, prepared_package_by_name) =
    prepare_packages_for_publishing(&cli_factory, deno_json, import_map)
      .await?;

  if prepared_package_by_name.is_empty() {
    bail!("No packages to publish");
  }

  perform_publish(
    cli_factory.http_client(),
    publish_order_graph,
    prepared_package_by_name,
    auth_method,
  )
  .await
}
