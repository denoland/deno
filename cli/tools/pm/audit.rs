// Copyright 2018-2026 the Deno authors. MIT license.

use std::io::Write;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_npm::resolution::NpmResolutionSnapshot;
use eszip::v2::Url;
use http::header::HeaderName;
use http::header::HeaderValue;
use serde::Deserialize;

use crate::args::AuditFlags;
use crate::args::Flags;
use crate::colors;
use crate::factory::CliFactory;
use crate::http_util;
use crate::http_util::HttpClient;
use crate::http_util::HttpClientProvider;

pub async fn audit(
  flags: Arc<Flags>,
  audit_flags: AuditFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags);
  let npm_resolver = factory.npm_resolver().await?;
  let npm_resolver = npm_resolver.as_managed().unwrap();
  let snapshot = npm_resolver.resolution().snapshot();

  let npm_url = &factory.npmrc()?.default_config.registry_url;
  let http_provider = HttpClientProvider::new(None, None);
  let http_client = http_provider
    .get_or_create()
    .context("Failed to create HTTP client")?;

  let use_socket = audit_flags.socket;

  let r =
    npm::call_audits_api(audit_flags, npm_url, &snapshot, http_client).await?;

  if use_socket {
    socket_dev::call_firewall_api(
      &snapshot,
      http_provider.get_or_create().unwrap(),
    )
    .await?;
  }

  Ok(r)
}

mod npm {
  use std::collections::HashMap;
  use std::collections::HashSet;

  use super::*;

  #[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
  enum AdvisorySeverity {
    Low,
    Moderate,
    High,
    Critical,
  }

  impl AdvisorySeverity {
    fn parse(str_: &str) -> Option<Self> {
      match str_ {
        "low" => Some(Self::Low),
        "moderate" => Some(Self::Moderate),
        "high" => Some(Self::High),
        "critical" => Some(Self::Critical),
        _ => None,
      }
    }
  }

  pub async fn call_audits_api_inner(
    client: &HttpClient,
    npm_url: Url,
    body: serde_json::Value,
  ) -> Result<BulkAuditResponse, AnyError> {
    let url = npm_url.join("-/npm/v1/security/advisories/bulk").unwrap();
    let future = client.post_json(url, &body)?.send().boxed_local();
    let response = future.await?;
    let json_str = http_util::body_to_string(response)
      .await
      .context("Failed to read response from the npm registry API")?;
    let response: BulkAuditResponse = serde_json::from_str(&json_str)
      .context("Failed to deserialize response from the npm registry API")?;
    Ok(response)
  }

  pub async fn call_audits_api(
    audit_flags: AuditFlags,
    npm_url: &Url,
    npm_resolution_snapshot: &NpmResolutionSnapshot,
    client: HttpClient,
  ) -> Result<i32, AnyError> {
    // Build request body for the bulk advisory endpoint:
    // { "pkg-name": ["ver1", "ver2"], ... }
    let mut body_map: HashMap<String, HashSet<String>> = HashMap::new();
    for pkg in npm_resolution_snapshot.all_packages_for_every_system() {
      body_map
        .entry(pkg.id.nv.name.to_string())
        .or_default()
        .insert(pkg.id.nv.version.to_string());
    }
    let body: HashMap<String, Vec<String>> = body_map
      .into_iter()
      .map(|(k, v)| (k, v.into_iter().collect()))
      .collect();
    let body = serde_json::to_value(&body).unwrap();

    let bulk_response =
      match call_audits_api_inner(&client, npm_url.clone(), body).await {
        Ok(s) => s,
        Err(err) => {
          if audit_flags.ignore_registry_errors {
            log::error!("Failed to get data from the registry: {}", err);
            return Ok(0);
          } else {
            return Err(err);
          }
        }
      };

    // Convert bulk response to flat list of advisories
    let mut advisories: Vec<AuditAdvisory> = Vec::new();
    for (pkg_name, pkg_advisories) in &bulk_response {
      for adv in pkg_advisories {
        advisories.push(AuditAdvisory {
          title: adv.title.clone(),
          severity: adv.severity.clone(),
          url: adv.url.clone(),
          module_name: pkg_name.clone(),
          vulnerable_versions: adv.vulnerable_versions.clone(),
          patched_versions: adv.patched_versions.clone().unwrap_or_default(),
          cves: adv.cves.clone(),
        });
      }
    }

    // Filter out advisories where no installed version falls within
    // the vulnerable range. This handles package.json overrides that
    // force a patched version.
    {
      let mut installed_versions: HashMap<String, Vec<deno_semver::Version>> =
        HashMap::new();
      for pkg in npm_resolution_snapshot.all_packages_for_every_system() {
        installed_versions
          .entry(pkg.id.nv.name.to_string())
          .or_default()
          .push(pkg.id.nv.version.clone());
      }
      advisories.retain(|adv| {
        let Ok(vulnerable_range) =
          deno_semver::VersionReq::parse_from_npm(&adv.vulnerable_versions)
        else {
          // Can't parse the range; keep the advisory to be safe
          return true;
        };
        if let Some(versions) = installed_versions.get(&adv.module_name) {
          versions.iter().any(|v| vulnerable_range.matches(v))
        } else {
          false
        }
      });
    }

    // Filter out ignored CVEs
    if !audit_flags.ignore.is_empty() {
      advisories.retain(|adv| {
        !adv.cves.iter().any(|cve| audit_flags.ignore.contains(cve))
      });
    }

    // Compute vulnerability counts from remaining advisories
    let mut vulns = AuditVulnerabilities {
      low: 0,
      moderate: 0,
      high: 0,
      critical: 0,
    };
    for adv in &advisories {
      match AdvisorySeverity::parse(&adv.severity) {
        Some(AdvisorySeverity::Low) => vulns.low += 1,
        Some(AdvisorySeverity::Moderate) => vulns.moderate += 1,
        Some(AdvisorySeverity::High) => vulns.high += 1,
        Some(AdvisorySeverity::Critical) => vulns.critical += 1,
        None => {}
      }
    }

    if vulns.total() == 0 {
      _ = writeln!(&mut std::io::stdout(), "No known vulnerabilities found",);
      return Ok(0);
    }

    advisories.sort_by_cached_key(|adv| {
      format!("{}@{}", adv.module_name, adv.vulnerable_versions)
    });

    let minimal_severity =
      AdvisorySeverity::parse(&audit_flags.severity).unwrap();
    print_report(
      &vulns,
      &advisories,
      minimal_severity,
      audit_flags.ignore_unfixable,
    );

    // Exit code 1 only if there are vulnerabilities at or above the specified level
    let exit_code = if vulns.count_at_or_above(minimal_severity) > 0 {
      1
    } else {
      0
    };
    Ok(exit_code)
  }

  fn print_report(
    vulns: &AuditVulnerabilities,
    advisories: &[AuditAdvisory],
    minimal_severity: AdvisorySeverity,
    ignore_unfixable: bool,
  ) {
    let stdout = &mut std::io::stdout();

    for adv in advisories {
      let Some(severity) = AdvisorySeverity::parse(&adv.severity) else {
        continue;
      };
      if severity < minimal_severity {
        continue;
      }

      let has_fix = !adv.patched_versions.is_empty();
      if !has_fix && ignore_unfixable {
        continue;
      }

      _ = writeln!(stdout, "╭ {}", colors::bold(adv.title.to_string()));
      _ = writeln!(
        stdout,
        "│ {}   {}",
        colors::gray("Severity:"),
        match severity {
          AdvisorySeverity::Low => colors::bold("low"),
          AdvisorySeverity::Moderate => colors::yellow("moderate"),
          AdvisorySeverity::High => colors::red("high"),
          AdvisorySeverity::Critical => colors::red("critical"),
        }
      );
      _ = writeln!(
        stdout,
        "│ {}    {}",
        colors::gray("Package:"),
        adv.module_name
      );
      _ = writeln!(
        stdout,
        "│ {} {}",
        colors::gray("Vulnerable:"),
        adv.vulnerable_versions
      );
      if has_fix {
        _ = writeln!(
          stdout,
          "│ {}    {}",
          colors::gray("Patched:"),
          adv.patched_versions
        );
        _ = writeln!(stdout, "│ {}       {}", colors::gray("Info:"), adv.url);
        _ = writeln!(
          stdout,
          "╰ {}    update {} to {}",
          colors::gray("Actions:"),
          adv.module_name,
          adv.patched_versions
        );
      } else {
        _ = writeln!(stdout, "╰ {}       {}", colors::gray("Info:"), adv.url);
      }
      _ = writeln!(stdout);
    }

    _ = writeln!(
      stdout,
      "Found {} vulnerabilities",
      colors::red(vulns.total()),
    );
    _ = writeln!(
      stdout,
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

  /// Advisory item from the bulk API response.
  #[derive(Debug, Deserialize)]
  pub struct BulkAdvisoryItem {
    pub url: String,
    pub title: String,
    pub severity: String,
    pub vulnerable_versions: String,
    #[serde(default)]
    pub patched_versions: Option<String>,
    #[serde(default)]
    pub cves: Vec<String>,
    #[serde(default)]
    #[allow(dead_code, reason = "deserialized but not yet displayed")]
    pub cwe: Vec<String>,
  }

  /// The bulk advisory endpoint response: { "package-name": [advisory, ...] }
  pub type BulkAuditResponse = HashMap<String, Vec<BulkAdvisoryItem>>;

  /// Internal advisory representation with module name from the response key.
  struct AuditAdvisory {
    title: String,
    severity: String,
    url: String,
    module_name: String,
    vulnerable_versions: String,
    patched_versions: String,
    cves: Vec<String>,
  }

  struct AuditVulnerabilities {
    low: i32,
    moderate: i32,
    high: i32,
    critical: i32,
  }

  impl AuditVulnerabilities {
    fn total(&self) -> i32 {
      self.low + self.moderate + self.high + self.critical
    }

    fn count_at_or_above(&self, min_severity: AdvisorySeverity) -> i32 {
      match min_severity {
        AdvisorySeverity::Low => self.total(),
        AdvisorySeverity::Moderate => self.moderate + self.high + self.critical,
        AdvisorySeverity::High => self.high + self.critical,
        AdvisorySeverity::Critical => self.critical,
      }
    }
  }
}

mod socket_dev {
  use super::*;

  pub async fn call_firewall_api(
    npm_resolution_snapshot: &NpmResolutionSnapshot,
    client: HttpClient,
  ) -> Result<(), AnyError> {
    let purls = npm_resolution_snapshot
      .all_packages_for_every_system()
      .map(|package| {
        format!("pkg:npm/{}@{}", package.id.nv.name, package.id.nv.version)
      })
      .collect::<Vec<_>>();

    let api_key = std::env::var("SOCKET_API_KEY").ok();

    let mut purl_responses = if let Some(api_key) = api_key {
      call_authenticated_api(&client, &purls, &api_key).await?
    } else {
      call_unauthenticated_api(&client, &purls).await?
    };

    purl_responses.sort_by_cached_key(|r| r.name.to_string());

    print_firewall_report(&purl_responses);

    Ok(())
  }

  async fn call_authenticated_api(
    client: &HttpClient,
    purls: &[String],
    api_key: &str,
  ) -> Result<Vec<FirewallResponse>, AnyError> {
    let socket_dev_url =
      std::env::var("SOCKET_DEV_URL").ok().unwrap_or_else(|| {
        "https://api.socket.dev/v0/purl?actions=error,warn".to_string()
      });
    let url = Url::parse(&socket_dev_url).unwrap();

    let body = serde_json::json!({
      "components": purls.iter().map(|purl| {
        serde_json::json!({ "purl": purl })
      }).collect::<Vec<_>>()
    });

    let auth_value = HeaderValue::from_str(&format!("Bearer {}", api_key))
      .context("Failed to create Authorization header")?;

    let request = client
      .post_json(url, &body)?
      .header(HeaderName::from_static("authorization"), auth_value);

    let response = request.send().boxed_local().await?;
    let text = http_util::body_to_string(response).await?;

    // Response is nJSON
    let responses = text
      .lines()
      .filter(|line| !line.trim().is_empty())
      .map(|line| {
        serde_json::from_str::<FirewallResponse>(line)
          .context("Failed to parse Socket.dev response")
      })
      .collect::<Result<Vec<_>, _>>()?;

    Ok(responses)
  }

  async fn call_unauthenticated_api(
    client: &HttpClient,
    purls: &[String],
  ) -> Result<Vec<FirewallResponse>, AnyError> {
    let socket_dev_url = std::env::var("SOCKET_DEV_URL")
      .ok()
      .unwrap_or_else(|| "https://firewall-api.socket.dev/".to_string());

    let futures = purls
      .iter()
      .map(|purl| {
        let url = Url::parse(&format!(
          "{}purl/{}",
          socket_dev_url,
          percent_encoding::utf8_percent_encode(
            purl,
            percent_encoding::NON_ALPHANUMERIC
          )
        ))
        .unwrap();
        client.download_text(url).boxed_local()
      })
      .collect::<Vec<_>>();

    let purl_results = futures::stream::iter(futures)
      .buffer_unordered(20)
      .collect::<Vec<_>>()
      .await;

    let responses = purl_results
      .into_iter()
      .filter_map(|result| match result {
        Ok(a) => Some(a),
        Err(err) => {
          log::error!("Failed to get PURL result {:?}", err);
          None
        }
      })
      .filter_map(|json_response| {
        match serde_json::from_str::<FirewallResponse>(&json_response) {
          Ok(response) => Some(response),
          Err(err) => {
            log::error!("Failed deserializing socket.dev response {:?}", err);
            None
          }
        }
      })
      .collect::<Vec<_>>();

    Ok(responses)
  }

  fn print_firewall_report(responses: &[FirewallResponse]) {
    let stdout = &mut std::io::stdout();

    let responses_with_alerts = responses
      .iter()
      .filter(|r| !r.alerts.is_empty())
      .collect::<Vec<_>>();

    if responses_with_alerts.is_empty() {
      return;
    }

    _ = writeln!(stdout);
    _ = writeln!(stdout, "{}", colors::bold("Socket.dev firewall report"));
    _ = writeln!(stdout);

    // Count total alerts by severity
    let mut total_critical = 0;
    let mut total_high = 0;
    let mut total_medium = 0;
    let mut total_low = 0;
    let mut packages_with_issues = 0;

    for response in responses_with_alerts {
      packages_with_issues += 1;

      _ = writeln!(stdout, "╭ pkg:npm/{}@{}", response.name, response.version);

      if let Some(score) = &response.score {
        _ = writeln!(
          stdout,
          "│ {:<20} {:>3}",
          colors::gray("Supply Chain Risk:"),
          format_score(score.supply_chain)
        );
        _ = writeln!(
          stdout,
          "│ {:<20} {:>3}",
          colors::gray("Maintenance:"),
          format_score(score.maintenance)
        );
        _ = writeln!(
          stdout,
          "│ {:<20} {:>3}",
          colors::gray("Quality:"),
          format_score(score.quality)
        );
        _ = writeln!(
          stdout,
          "│ {:<20} {:>3}",
          colors::gray("Vulnerabilities:"),
          format_score(score.vulnerability)
        );
        _ = writeln!(
          stdout,
          "│ {:<20} {:>3}",
          colors::gray("License:"),
          format_score(score.license)
        );
      }

      // critical and high are counted as one for display.
      let mut critical_count = 0;
      let mut medium_count = 0;
      let mut low_count = 0;

      for alert in &response.alerts {
        match alert.severity.as_str() {
          "critical" => {
            total_critical += 1;
            critical_count += 1;
          }
          "high" => {
            total_high += 1;
            critical_count += 1;
          }
          "medium" => {
            total_medium += 1;
            medium_count += 1;
          }
          "low" => {
            total_low += 1;
            low_count += 1;
          }
          _ => {}
        }
      }

      if !response.alerts.is_empty() {
        let alerts_str = response
          .alerts
          .iter()
          .map(|alert| {
            let severity_bracket = match alert.severity.as_str() {
              "critical" => colors::red("critical").to_string(),
              "high" => colors::red("high").to_string(),
              "medium" => colors::yellow("medium").to_string(),
              "low" => "low".to_string(),
              _ => alert.severity.clone(),
            };
            format!("[{}] {}", severity_bracket, alert.r#type)
          })
          .collect::<Vec<_>>()
          .join(", ");

        let label = format!(
          "Alerts ({}/{}/{}):",
          critical_count, medium_count, low_count
        );
        _ = writeln!(stdout, "╰ {:<20} {}", colors::gray(&label), alerts_str);
      } else {
        _ = writeln!(stdout, "╰");
      }
      _ = writeln!(stdout);
    }

    let total_alerts = total_critical + total_high + total_medium + total_low;

    if total_alerts == 0 && packages_with_issues == 0 {
      _ = writeln!(stdout, "No security alerts found from Socket.dev");
      return;
    }

    if total_alerts > 0 {
      _ = writeln!(
        stdout,
        "Found {} alerts across {} packages",
        colors::red(total_alerts),
        colors::bold(packages_with_issues)
      );
      _ = writeln!(
        stdout,
        "Severity: {} {}, {} {}, {} {}, {} {}",
        colors::bold(total_low),
        colors::bold("low"),
        colors::yellow(total_medium),
        colors::yellow("medium"),
        colors::red(total_high),
        colors::red("high"),
        colors::red(total_critical),
        colors::red("critical"),
      );
    }
  }

  fn format_score(score: f64) -> String {
    let percentage = (score * 100.0) as i32;
    let colored = if percentage >= 80 {
      colors::green(percentage)
    } else if percentage >= 60 {
      colors::yellow(percentage)
    } else {
      colors::red(percentage)
    };
    format!("{}", colored)
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct FirewallScore {
    pub license: f64,
    pub maintenance: f64,
    #[allow(dead_code, reason = "we don't use it yet")]
    pub overall: f64,
    pub quality: f64,
    pub supply_chain: f64,
    pub vulnerability: f64,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct FirewallAlert {
    pub r#type: String,
    #[allow(dead_code, reason = "we don't use it yet")]
    pub action: String,
    pub severity: String,
    #[allow(dead_code, reason = "we don't use it yet")]
    pub category: String,
  }

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  pub struct FirewallResponse {
    #[allow(dead_code, reason = "we don't use it yet")]
    pub id: String,
    pub name: String,
    pub version: String,
    pub score: Option<FirewallScore>,
    #[serde(default)]
    pub alerts: Vec<FirewallAlert>,
  }
}

#[cfg(test)]
mod tests {
  use deno_core::serde_json;

  use super::npm::BulkAuditResponse;

  #[test]
  fn test_bulk_audit_response_deserialize_empty() {
    let json = r#"{}"#;
    let response: BulkAuditResponse = serde_json::from_str(json).unwrap();
    assert!(response.is_empty());
  }

  #[test]
  fn test_bulk_audit_response_deserialize_with_advisory() {
    let json = r#"{
      "@denotest/with-vuln1": [{
        "url": "https://example.com/vuln/101010",
        "title": "test vulnerability",
        "severity": "high",
        "vulnerable_versions": "<1.1.0"
      }]
    }"#;
    let response: BulkAuditResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.len(), 1);
    let advisories = &response["@denotest/with-vuln1"];
    assert_eq!(advisories.len(), 1);
    assert_eq!(advisories[0].severity, "high");
    assert!(advisories[0].patched_versions.is_none());
    assert!(advisories[0].cves.is_empty());
  }

  #[test]
  fn test_bulk_audit_response_deserialize_with_optional_fields() {
    let json = r#"{
      "test-pkg": [{
        "url": "https://example.com",
        "title": "test",
        "severity": "critical",
        "vulnerable_versions": "<2.0.0",
        "patched_versions": ">=2.0.0",
        "cves": ["CVE-2025-0001"],
        "cwe": ["CWE-1333"]
      }]
    }"#;
    let response: BulkAuditResponse = serde_json::from_str(json).unwrap();
    let advisories = &response["test-pkg"];
    assert_eq!(advisories[0].patched_versions.as_deref(), Some(">=2.0.0"));
    assert_eq!(advisories[0].cves, vec!["CVE-2025-0001"]);
    assert_eq!(advisories[0].cwe, vec!["CWE-1333"]);
  }
}
