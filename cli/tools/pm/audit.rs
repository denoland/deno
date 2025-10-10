// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::futures::future::join_all;
use deno_core::serde_json;
use deno_npm::resolution::NpmResolutionSnapshot;
use eszip::v2::Url;
use serde::Deserialize;
use serde::Serialize;

use crate::args::AuditFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::http_util;
use crate::http_util::HttpClient;
use crate::http_util::HttpClientProvider;

pub async fn audit(
  flags: Arc<Flags>,
  _audit_flags: AuditFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let _cli_options = factory.cli_options()?;
  let npm_resolver = factory.npm_resolver().await?;
  let npm_resolver = npm_resolver.as_managed().unwrap();
  let snapshot = npm_resolver.resolution().snapshot();

  let http_provider = HttpClientProvider::new(None, None);
  let _npm_response =
    npm::call_audits_api(&snapshot, http_provider.get_or_create().unwrap())
      .await?;

  // let _purl_responses = socket_dev::call_firewall_api(
  //   &snapshot,
  //   http_provider.get_or_create().unwrap(),
  // )
  // .await?;

  // for response in purl_responses {
  //   if let Some(score) = response.score {
  //     if score.overall <= 0.2 {
  //       eprintln!(
  //         "{}@{} Low score - {}",
  //         response.name, response.version, score.overall
  //       );
  //     }
  //   }
  //   if !response.alerts.is_empty() {
  //     for alert in response.alerts.iter() {
  //       eprintln!(
  //         "{}@{} Alert - {} - {}",
  //         response.name, response.version, alert.severity, alert.category
  //       );
  //     }
  //   }
  // }

  Ok(())
}

mod npm {
  use std::collections::HashMap;

  use super::*;

  pub async fn call_audits_api(
    npm_resolution_snapshot: &NpmResolutionSnapshot,
    client: HttpClient,
  ) -> Result<(), AnyError> {
    let top_level_packages = npm_resolution_snapshot.top_level_packages();
    let mut requires = HashMap::with_capacity(top_level_packages.len());
    let mut dependencies = HashMap::with_capacity(top_level_packages.len());
    for package in top_level_packages {
      requires
        .insert(package.nv.name.to_string(), package.nv.version.to_string());
      dependencies.insert(
        package.nv.name.to_string(),
        Box::new(DependencyDescriptor {
          version: package.nv.version.to_string(),
          // TODO
          dev: false,
          // TODO
          dependencies: vec![],
        }),
      );
    }

    let body = serde_json::json!({
        "dev": false,
        "install": [],
        "metadata": {},
        "remove": [],
        "requires": requires,
        "dependencies": dependencies,
    });

    eprintln!("body {}", serde_json::to_string_pretty(&body).unwrap());
    let url = Url::parse("https://registry.npmjs.org/-/npm/v1/security/audits")
      .unwrap();
    let future = client.post_json(url, &body)?.send().boxed_local();
    let response = future.await?;
    let json_str = http_util::body_to_string(response).await.unwrap();
    dbg!(&json_str);
    let json_obj: serde_json::Value = serde_json::from_str(&json_str)?;
    dbg!(&json_obj);
    Ok(())
  }

  #[derive(Debug, Serialize)]
  #[serde(rename_all = "camelCase")]

  struct DependencyDescriptor {
    version: String,
    dev: bool,
    dependencies: Vec<Box<DependencyDescriptor>>,
  }
}

mod socket_dev {
  use super::*;

  pub async fn call_firewall_api(
    npm_resolution_snapshot: &NpmResolutionSnapshot,
    client: HttpClient,
  ) -> Result<Vec<FirewallResponse>, AnyError> {
    let purls = npm_resolution_snapshot
      .all_packages_for_every_system()
      .map(|package| {
        format!("pkg:npm/{}@{}", package.id.nv.name, package.id.nv.version)
      })
      .collect::<Vec<_>>();

    eprintln!("purls {:#?}", purls);

    let futures = purls
      .into_iter()
      .map(|purl| {
        let url = Url::parse(&format!(
          "https://firewall-api.socket.dev/purl/{}",
          percent_encoding::utf8_percent_encode(
            &purl,
            percent_encoding::NON_ALPHANUMERIC
          )
        ))
        .unwrap();
        client.download_text(url).boxed_local()
      })
      .collect::<Vec<_>>();

    // TODO(bartlomieju): run at most 20 requests at the same time, waiting on socket.dev
    // to provide a batch API
    let purl_results = join_all(futures).await;

    let mut purl_responses = purl_results
      .into_iter()
      .filter_map(|result| match result {
        Ok(a) => Some(a),
        Err(err) => {
          eprintln!("Failed to get result {:?}", err);
          None
        }
      })
      .map(|json_response| {
        let response: FirewallResponse =
          serde_json::from_str(&json_response).unwrap();
        response
      })
      .collect::<Vec<_>>();
    purl_responses.sort_by_cached_key(|r| r.name.to_string());

    Ok(purl_responses)
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct FirewallScore {
    pub license: f64,
    pub maintenance: f64,
    pub overall: f64,
    pub quality: f64,
    pub supply_chain: f64,
    pub vulnerability: f64,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct FirewallAlert {
    pub r#type: String,
    pub action: String,
    pub severity: String,
    pub category: String,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct FirewallResponse {
    pub id: String,
    pub name: String,
    pub version: String,
    pub score: Option<FirewallScore>,
    #[serde(default)]
    pub alerts: Vec<FirewallAlert>,
  }
}
