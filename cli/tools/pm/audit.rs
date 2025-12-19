// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Write;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_resolver::npmrc::npm_registry_url;
use eszip::v2::Url;
use http::header::HeaderName;
use http::header::HeaderValue;
use serde::Deserialize;
use serde::Serialize;

use crate::args::AuditFlags;
use crate::args::Flags;
use crate::colors;
use crate::factory::CliFactory;
use crate::http_util;
use crate::http_util::HttpClient;
use crate::http_util::HttpClientProvider;
use crate::sys::CliSys;

pub async fn audit(
  flags: Arc<Flags>,
  audit_flags: AuditFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags);
  let workspace = factory.workspace_resolver().await?;
  let npm_resolver = factory.npm_resolver().await?;
  let npm_resolver = npm_resolver.as_managed().unwrap();
  let snapshot = npm_resolver.resolution().snapshot();

  let sys = CliSys::default();
  let npm_url = npm_registry_url(&sys);
  let http_provider = HttpClientProvider::new(None, None);
  let http_client = http_provider
    .get_or_create()
    .context("Failed to create HTTP client")?;

  let use_socket = audit_flags.socket;

  let r = npm::call_audits_api(
    audit_flags,
    npm_url,
    workspace,
    &snapshot,
    http_client,
  )
  .await?;

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

  use deno_npm::NpmPackageId;
  use deno_package_json::PackageJsonDepValue;
  use deno_resolver::workspace::WorkspaceResolver;
  use deno_semver::package::PackageNv;

  use super::*;
  use crate::sys::CliSys;

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

  fn get_dependency_descriptors_for_deps(
    seen: &mut HashSet<PackageNv>,
    all_dependencies_snapshot: &NpmResolutionSnapshot,
    dev_dependencies_snapshot: &NpmResolutionSnapshot,
    package_id: &NpmPackageId,
  ) -> HashMap<String, Box<DependencyDescriptor>> {
    let mut is_dev = false;

    let resolution_package =
      match dev_dependencies_snapshot.package_from_id(package_id) {
        Some(p) => {
          is_dev = true;
          p
        }
        None => all_dependencies_snapshot
          .package_from_id(package_id)
          .unwrap(),
      };
    let mut deps_map =
      HashMap::with_capacity(resolution_package.dependencies.len());
    for dep in resolution_package.dependencies.iter() {
      if !seen.insert(dep.1.nv.clone()) {
        continue;
      }

      let dep_deps = get_dependency_descriptors_for_deps(
        seen,
        all_dependencies_snapshot,
        dev_dependencies_snapshot,
        dep.1,
      );
      deps_map.insert(
        dep.0.to_string(),
        Box::new(DependencyDescriptor {
          version: dep.1.nv.version.to_string(),
          dev: is_dev,
          requires: dep_deps
            .iter()
            .map(|(k, v)| (k.to_string(), v.version.to_string()))
            .collect(),
          dependencies: dep_deps,
        }),
      );
    }
    deps_map
  }

  pub async fn call_audits_api_inner(
    client: &HttpClient,
    npm_url: Url,
    body: serde_json::Value,
  ) -> Result<AuditResponse, AnyError> {
    let url = npm_url.join("/-/npm/v1/security/audits").unwrap();
    let future = client.post_json(url, &body)?.send().boxed_local();
    let response = future.await?;
    let json_str = http_util::body_to_string(response)
      .await
      .context("Failed to read response from the npm registry API")?;
    let response: AuditResponse = serde_json::from_str(&json_str)
      .context("Failed to deserialize response from the npm registry API")?;
    Ok(response)
  }

  /// Partition into as few groups as possible so that no partition
  /// contains two entries with the same `name`.
  pub fn partition_packages<'a>(
    pkgs: &'a [&NpmPackageId],
  ) -> Vec<Vec<&'a NpmPackageId>> {
    // 1) Group by name
    let mut by_name: HashMap<&str, Vec<&NpmPackageId>> = HashMap::new();
    for p in pkgs {
      by_name.entry(&p.nv.name[..]).or_default().push(p);
    }

    // 2) The minimal number of partitions is the max multiplicity per name
    let k = by_name.values().map(|v| v.len()).max().unwrap_or(0);
    if k == 0 {
      return Vec::new();
    }

    // 3) Create k partitions
    let mut partitions: Vec<Vec<&NpmPackageId>> = vec![Vec::new(); k];

    // 4) Round-robin each name-group across the partitions
    for group in by_name.values() {
      for (i, item) in group.iter().enumerate() {
        partitions[i].push(*item);
      }
    }

    partitions
  }

  /// Merges multiple audit responses into a single consolidated response
  fn merge_responses(responses: Vec<AuditResponse>) -> AuditResponse {
    let mut merged_advisories = HashMap::new();
    let mut merged_actions = Vec::new();
    let mut total_low = 0;
    let mut total_moderate = 0;
    let mut total_high = 0;
    let mut total_critical = 0;

    for response in responses {
      // Merge advisories (HashMap by advisory ID)
      for (id, advisory) in response.advisories {
        merged_advisories.insert(id, advisory);
      }

      // Merge actions
      merged_actions.extend(response.actions);

      // Sum up vulnerability counts
      total_low += response.metadata.vulnerabilities.low;
      total_moderate += response.metadata.vulnerabilities.moderate;
      total_high += response.metadata.vulnerabilities.high;
      total_critical += response.metadata.vulnerabilities.critical;
    }

    AuditResponse {
      advisories: merged_advisories,
      actions: merged_actions,
      metadata: AuditMetadata {
        vulnerabilities: AuditVulnerabilities {
          low: total_low,
          moderate: total_moderate,
          high: total_high,
          critical: total_critical,
        },
      },
    }
  }

  pub async fn call_audits_api(
    audit_flags: AuditFlags,
    npm_url: Url,
    workspace: &WorkspaceResolver<CliSys>,
    npm_resolution_snapshot: &NpmResolutionSnapshot,
    client: HttpClient,
  ) -> Result<i32, AnyError> {
    let top_level_packages = npm_resolution_snapshot
      .top_level_packages()
      .collect::<Vec<_>>();
    // In deno.json users might define two different versions of the same package - so we need
    // to partition top level packages into buckets, to check all versions used.
    let top_level_packages_partitions = partition_packages(&top_level_packages);

    let mut requires = HashMap::with_capacity(top_level_packages.len());
    let mut dependencies = HashMap::with_capacity(top_level_packages.len());

    // Collect all dev dependencies, so they can be properly marked in the request body - since
    // there's no way to specify `devDependencies` in `deno.json`, this is only iterating
    // through discovered `package.json` files.
    let mut all_dev_deps = Vec::with_capacity(32);
    for pkg_json in workspace.package_jsons() {
      let deps = pkg_json.resolve_local_package_json_deps();
      for v in deps.dev_dependencies.values() {
        let Ok(PackageJsonDepValue::Req(package_req)) = v else {
          continue;
        };
        all_dev_deps.push(package_req.clone());
      }
    }
    let dev_dependencies_snapshot =
      npm_resolution_snapshot.subset(&all_dev_deps);

    let mut responses = Vec::with_capacity(top_level_packages_partitions.len());
    // And now let's construct the request body we need for the npm audits API.
    let seen = &mut HashSet::with_capacity(top_level_packages.len() * 100);
    for partition in top_level_packages_partitions {
      for package in partition {
        let is_dev =
          dev_dependencies_snapshot.package_from_id(package).is_some();
        requires
          .insert(package.nv.name.to_string(), package.nv.version.to_string());
        seen.insert(package.nv.clone());
        let package_deps = get_dependency_descriptors_for_deps(
          seen,
          npm_resolution_snapshot,
          &dev_dependencies_snapshot,
          package,
        );
        dependencies.insert(
          package.nv.name.to_string(),
          Box::new(DependencyDescriptor {
            version: package.nv.version.to_string(),
            dev: is_dev,
            requires: package_deps
              .iter()
              .map(|(k, v)| (k.to_string(), v.version.to_string()))
              .collect(),
            dependencies: package_deps,
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

      let r = call_audits_api_inner(&client, npm_url.clone(), body).await;
      let audit_response: AuditResponse = match r {
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
      responses.push(audit_response);
    }

    // Merge all responses into a single response
    let response = merge_responses(responses);

    let vulns = response.metadata.vulnerabilities;
    if vulns.total() == 0 {
      _ = writeln!(&mut std::io::stdout(), "No known vulnerabilities found",);
      return Ok(0);
    }

    let mut advisories = response.advisories.values().collect::<Vec<_>>();
    advisories.sort_by_cached_key(|adv| {
      format!("{}@{}", adv.module_name, adv.vulnerable_versions)
    });

    let minimal_severity =
      AdvisorySeverity::parse(&audit_flags.severity).unwrap();
    print_report(
      vulns,
      advisories,
      response.actions,
      minimal_severity,
      audit_flags.ignore_unfixable,
    );

    Ok(1)
  }

  fn print_report(
    vulns: AuditVulnerabilities,
    advisories: Vec<&AuditAdvisory>,
    actions: Vec<AuditAction>,
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

      let actions = adv.find_actions(&actions);
      if actions.is_empty() && ignore_unfixable {
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
      _ = writeln!(
        stdout,
        "│ {}    {}",
        colors::gray("Patched:"),
        adv.patched_versions
      );
      if let Some(finding) = adv.findings.first()
        && let Some(path) = finding.paths.first()
      {
        _ = writeln!(stdout, "│ {}       {}", colors::gray("Path:"), path);
      }
      if actions.is_empty() {
        _ = writeln!(stdout, "╰ {}      {}", colors::gray("Info:"), adv.url);
      } else {
        _ = writeln!(stdout, "│ {}       {}", colors::gray("Info:"), adv.url);
      }
      if actions.len() == 1 {
        _ =
          writeln!(stdout, "╰ {}    {}", colors::gray("Actions:"), actions[0]);
      } else if actions.len() > 1 {
        _ =
          writeln!(stdout, "│ {}    {}", colors::gray("Actions:"), actions[0]);
        for action in &actions[0..actions.len() - 2] {
          _ = writeln!(stdout, "│             {}", action);
        }
        _ = writeln!(stdout, "╰             {}", actions[actions.len() - 1]);
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

  #[derive(Debug, Serialize)]
  #[serde(rename_all = "camelCase")]
  struct DependencyDescriptor {
    version: String,
    dev: bool,
    requires: HashMap<String, String>,
    dependencies: HashMap<String, Box<DependencyDescriptor>>,
  }

  #[derive(Debug, Deserialize)]
  pub struct AuditActionResolve {
    pub id: i32,
    pub path: Option<String>,
    // TODO(bartlomieju): currently not used, commented out so it's not flagged by clippy
    // pub dev: bool,
    // pub optional: bool,
    // pub bundled: bool,
  }

  #[derive(Debug, Deserialize)]
  pub struct AuditAction {
    #[serde(rename = "isMajor", default)]
    pub is_major: bool,
    pub action: String,
    pub resolves: Vec<AuditActionResolve>,
    pub module: Option<String>,
    pub target: Option<String>,
  }

  #[derive(Debug, Deserialize)]
  pub struct AdvisoryFinding {
    // TODO(bartlomieju): currently not used, commented out so it's not flagged by clippy
    // pub version: String,
    pub paths: Vec<String>,
  }

  #[derive(Debug, Deserialize)]
  pub struct AuditAdvisory {
    pub id: i32,
    pub title: String,
    pub findings: Vec<AdvisoryFinding>,
    // TODO(bartlomieju): currently not used, commented out so it's not flagged by clippy
    // pub cves: Vec<String>,
    // pub cwe: Vec<String>,
    pub severity: String,
    pub url: String,
    pub module_name: String,
    pub vulnerable_versions: String,
    pub patched_versions: String,
  }

  impl AuditAdvisory {
    fn find_actions(&self, actions: &[AuditAction]) -> Vec<String> {
      let mut acts = Vec::new();

      for action in actions {
        if !action.resolves.iter().any(|r| r.id == self.id) {
          continue;
        }

        let module = action
          .module
          .as_deref()
          .map(str::to_owned)
          .or_else(|| {
            // Fallback to infer from dependency path
            action.resolves.first().and_then(|r| {
              r.path
                .as_deref()
                .and_then(|p| p.split('>').next_back())
                .map(|s| s.trim().to_string())
            })
          })
          .unwrap_or_else(|| "<unknown>".to_string());

        let target = action
          .target
          .as_deref()
          .map(|t| format!("@{}", t))
          .unwrap_or_default();

        let major = if action.is_major {
          " (major upgrade)"
        } else {
          ""
        };

        acts.push(format!("{} {}{}{}", action.action, module, target, major));
      }

      acts
    }
  }

  #[derive(Debug, Deserialize)]
  pub struct AuditVulnerabilities {
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
    // TODO(bartlomieju): currently not used, commented out so it's not flagged by clippy
    // pub dependencies: i32,
    // pub dev_dependencies: i32,
    // pub optional_dependencies: i32,
    // pub total_dependencies: i32,
  }

  #[derive(Debug, Deserialize)]
  pub struct AuditResponse {
    pub actions: Vec<AuditAction>,
    pub advisories: HashMap<i32, AuditAdvisory>,
    pub metadata: AuditMetadata,
  }
}

mod socket_dev {
  #![allow(dead_code)]

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
