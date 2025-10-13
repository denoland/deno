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
use crate::colors;
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

  use deno_core::anyhow::Context;

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

    // eprintln!("body {}", serde_json::to_string_pretty(&body).unwrap());
    let url = Url::parse("https://registry.npmjs.org/-/npm/v1/security/audits")
      .unwrap();
    let future = client.post_json(url, &body)?.send().boxed_local();
    let response = future.await?;
    let json_str = http_util::body_to_string(response)
      .await
      .context("Failed to read response from the npm registry API")?;
    let response: AuditResponse = serde_json::from_str(&json_str)
      .context("Failed to deserialize response from the npm registry API")?;
    // dbg!(&response);

    print_report(response);

    Ok(())
  }

  fn print_report(response: AuditResponse) {
    let vulns = response.metadata.vulnerabilities;
    if vulns.total() == 0 {
      return;
    }

    let mut advisories = response.advisories.values().collect::<Vec<_>>();
    advisories.sort_by_cached_key(|adv| {
      format!("{}@{}", adv.module_name, adv.vulnerable_versions)
    });

    for adv in advisories {
      let actions = adv.find_actions(&response.actions);
      log::info!("╭ {}", colors::bold(adv.title.to_string()));
      log::info!(
        "│   {} {}",
        colors::gray("Severity:"),
        match adv.severity.as_str() {
          "low" => colors::bold("low").to_string(),
          "moderate" => colors::yellow("moderate").to_string(),
          "high" => colors::red("high").to_string(),
          "critical" => colors::red("critical").to_string(),
          sev => sev.to_string(),
        }
      );
      log::info!("│    {} {}", colors::gray("Package:"), adv.module_name);
      log::info!(
        "│ {} {}",
        colors::gray("Vulnerable:"),
        adv.vulnerable_versions
      );
      log::info!("│    {} {}", colors::gray("Patched:"), adv.patched_versions);
      if actions.is_empty() {
        log::info!("╰─      {} {}", colors::gray("Info:"), adv.url);
      } else {
        log::info!("│       {} {}", colors::gray("Info:"), adv.url);
      }
      if actions.len() == 1 {
        log::info!("╰─   {} {}", colors::gray("Actions:"), actions[0]);
      } else {
        log::info!("│    {} {}", colors::gray("Actions:"), actions[0]);
        for action in &actions[1..actions.len() - 2] {
          log::info!("│             {}", action);
        }
        log::info!("╰─            {}", actions[actions.len() - 1]);
      }
      log::info!("");
    }

    log::info!("Found {} vulnerabilities", colors::red(vulns.total()),);
    log::info!(
      "Severity: {} {}, {} {}, {} {}, {} {}",
      colors::bold(vulns.low),
      colors::bold("low"),
      colors::yellow(vulns.moderate),
      colors::yellow("moderate"),
      colors::red(vulns.high),
      colors::red("high"),
      colors::red(vulns.critical),
      colors::red("critical"),
    );
  }

  #[derive(Debug, Serialize)]
  #[serde(rename_all = "camelCase")]

  struct DependencyDescriptor {
    version: String,
    dev: bool,
    dependencies: Vec<Box<DependencyDescriptor>>,
  }

  #[derive(Debug, Deserialize)]
  pub struct AuditActionResolve {
    pub id: i32,
    pub path: String,
    pub dev: bool,
    pub optional: bool,
    pub bundled: bool,
  }

  #[derive(Debug, Deserialize)]
  pub struct AuditAction {
    #[serde(rename = "isMajor")]
    pub is_major: bool,
    pub action: String,
    pub resolves: Vec<AuditActionResolve>,
    pub module: String,
    pub target: String,
  }

  #[derive(Debug, Deserialize)]
  pub struct AuditAdvisory {
    pub id: i32,
    pub title: String,
    pub cves: Vec<String>,
    pub cwe: Vec<String>,
    pub severity: String,
    pub url: String,
    pub module_name: String,
    pub vulnerable_versions: String,
    pub patched_versions: String,
  }

  impl AuditAdvisory {
    fn find_actions(&self, actions: &[AuditAction]) -> Vec<String> {
      let mut acts = vec![];

      for action in actions {
        if action
          .resolves
          .iter()
          .any(|action_resolve| action_resolve.id == self.id)
        {
          acts.push(format!(
            "{} {}@{}{}",
            action.action,
            action.module,
            action.target,
            if action.is_major {
              " (major upgrade)"
            } else {
              ""
            }
          ))
        }
      }

      acts
    }
  }

  #[derive(Debug, Deserialize)]
  pub struct AuditVulnerabilities {
    pub info: i32,
    pub low: i32,
    pub moderate: i32,
    pub high: i32,
    pub critical: i32,
  }

  impl AuditVulnerabilities {
    fn total(&self) -> i32 {
      self.low + self.moderate + self.high + self.critical
    }
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct AuditMetadata {
    pub vulnerabilities: AuditVulnerabilities,
    pub dependencies: i32,
    pub dev_dependencies: i32,
    pub optional_dependencies: i32,
    pub total_dependencies: i32,
  }

  #[derive(Debug, Deserialize)]
  pub struct AuditResponse {
    pub actions: Vec<AuditAction>,
    pub advisories: HashMap<i32, AuditAdvisory>,
    pub metadata: AuditMetadata,
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
