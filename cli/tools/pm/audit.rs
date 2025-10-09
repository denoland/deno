// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::futures::future::join_all;
use deno_core::serde_json;
use eszip::v2::Url;
use serde::Deserialize;

use crate::args::AuditFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::http_util::HttpClientProvider;

pub async fn audit(
  flags: Arc<Flags>,
  audit_flags: AuditFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let npm_resolver = factory.npm_resolver().await?;
  let npm_resolver = npm_resolver.as_managed().unwrap();
  let snapshot = npm_resolver.resolution().snapshot();

  let http_provider = HttpClientProvider::new(None, None);
  let client = http_provider.get_or_create().unwrap();

  let purls = snapshot
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

  let purl_results = join_all(futures).await;
  // eprintln!("purl results {:#?}", purl_results);
  let mut purl_results = purl_results
    .into_iter()
    .filter_map(|result| match result {
      Ok(a) => Some(a),
      Err(err) => {
        eprintln!("Failed to get result {:?}", err);
        None
      }
    })
    .map(|json_response| {
      let response: SocketDevFirewallResponse =
        serde_json::from_str(&json_response).unwrap();
      response
    })
    .collect::<Vec<_>>();
  purl_results.sort_by_cached_key(|r| r.name.to_string());
  for response in purl_results {
    if let Some(score) = response.score {
      if score.overall <= 0.2 {
        eprintln!(
          "{}@{} Low score - {}",
          response.name, response.version, score.overall
        );
      }
    }
    if !response.alerts.is_empty() {
      for alert in response.alerts.iter() {
        eprintln!(
          "{}@{} Alert - {} - {}",
          response.name, response.version, alert.severity, alert.category
        );
      }
    }
  }
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SocketDevFirewallScore {
  license: f64,
  maintenance: f64,
  overall: f64,
  quality: f64,
  supply_chain: f64,
  vulnerability: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SocketDevFirewallAlert {
  r#type: String,
  action: String,
  severity: String,
  category: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SocketDevFirewallResponse {
  id: String,
  name: String,
  version: String,
  score: Option<SocketDevFirewallScore>,
  #[serde(default)]
  alerts: Vec<SocketDevFirewallAlert>,
}
