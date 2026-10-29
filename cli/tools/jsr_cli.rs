// Copyright 2018-2026 the Deno authors. MIT license.

//! `deno publish` and `deno pack` run the jsr CLI
//! (https://github.com/jsr-io/cli). Deno type checks the package, then
//! downloads the newest compatible jsr binary and runs it.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_config::workspace::JsrPackageConfig;
use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_semver::Version;
use deno_semver::VersionReq;
use sha2::Digest;

use crate::args::ConfigFlag;
use crate::args::Flags;
use crate::args::PackFlags;
use crate::args::PublishFlags;
use crate::factory::CliFactory;
use crate::graph_util::CreatePublishGraphOptions;
use crate::http_util::HttpClient;
use crate::tools::lint::collect_no_slow_type_diagnostics;
use crate::type_checker::CheckOptions;
use crate::util::archive;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressBarStyle;

/// The jsr CLI versions this Deno runs. Every version in the range must
/// accept the arguments built below.
const JSR_CLI_VERSION_REQ: &str = ">=0.15.0 <1.0.0";
const JSR_CLI_RELEASES_URL: &str = "https://github.com/jsr-io/cli/releases";

pub async fn publish(
  flags: Arc<Flags>,
  publish_flags: PublishFlags,
) -> Result<i32, AnyError> {
  let mut args = vec!["publish".to_string()];
  if let Some(token) = &publish_flags.token {
    args.extend(["--token".to_string(), token.clone()]);
  }
  push_flag(&mut args, "--dry-run", publish_flags.dry_run);
  push_flag(
    &mut args,
    "--allow-slow-types",
    publish_flags.allow_slow_types,
  );
  push_flag(&mut args, "--allow-dirty", publish_flags.allow_dirty);
  push_flag(&mut args, "--no-provenance", publish_flags.no_provenance);
  if let Some(version) = &publish_flags.set_version {
    args.extend(["--set-version".to_string(), version.clone()]);
  }
  push_shared_args(&mut args, &flags);

  let factory = CliFactory::from_flags(flags);
  type_check_packages(&factory, publish_flags.allow_slow_types, true).await?;
  run_jsr_cli(&factory, args).await
}

pub async fn pack(
  flags: Arc<Flags>,
  pack_flags: PackFlags,
) -> Result<i32, AnyError> {
  let mut args = vec!["pack".to_string()];
  if let Some(output) = &pack_flags.output {
    args.extend(["--output".to_string(), output.clone()]);
  }
  push_flag(&mut args, "--dry-run", pack_flags.dry_run);
  push_flag(&mut args, "--allow-slow-types", pack_flags.allow_slow_types);
  push_flag(&mut args, "--allow-dirty", pack_flags.allow_dirty);
  if let Some(version) = &pack_flags.set_version {
    args.extend(["--set-version".to_string(), version.clone()]);
  }
  push_flag(&mut args, "--no-source-maps", pack_flags.no_source_maps);
  if !pack_flags.files.ignore.is_empty() {
    args.push(format!("--ignore={}", pack_flags.files.ignore.join(",")));
  }
  push_shared_args(&mut args, &flags);
  if !pack_flags.files.include.is_empty() {
    args.push("--".to_string());
    args.extend(pack_flags.files.include.iter().cloned());
  }

  let factory = CliFactory::from_flags(flags);
  type_check_packages(&factory, pack_flags.allow_slow_types, false).await?;
  run_jsr_cli(&factory, args).await
}

fn push_flag(args: &mut Vec<String>, flag: &str, enabled: bool) {
  if enabled {
    args.push(flag.to_string());
  }
}

/// The config, unstable feature and log level flags.
fn push_shared_args(args: &mut Vec<String>, flags: &Flags) {
  match &flags.config_flag {
    ConfigFlag::Discover => {}
    ConfigFlag::Path(path) => {
      args.extend(["--config".to_string(), path.clone()]);
    }
    ConfigFlag::Disabled => args.push("--no-config".to_string()),
  }
  push_flag(
    args,
    "--unstable-sloppy-imports",
    flags.unstable_config.sloppy_imports,
  );
  push_flag(
    args,
    "--unstable-raw-imports",
    flags.unstable_config.raw_imports,
  );
  match flags.log_level {
    Some(log::Level::Error | log::Level::Warn) => args.push("--quiet".into()),
    Some(log::Level::Debug) => args.push("--log-level=debug".into()),
    Some(log::Level::Trace) => args.push("--log-level=trace".into()),
    Some(log::Level::Info) | None => {}
  }
}

/// Type checks the packages like `deno publish` and `deno pack` always did,
/// as the jsr CLI doesn't type check. Problems only the jsr CLI reports, like
/// slow types or a missing config file, are left to it.
async fn type_check_packages(
  factory: &CliFactory,
  allow_slow_types: bool,
  check_public_api: bool,
) -> Result<(), AnyError> {
  let cli_options = factory.cli_options()?;
  if !cli_options.type_check_mode().is_true() {
    return Ok(());
  }
  let packages = cli_options.start_dir.jsr_packages_for_publish();
  if packages.is_empty() {
    return Ok(());
  }

  // type checks the source of the packages
  let build_fast_check_graph = check_public_api && !allow_slow_types;
  let graph = factory
    .module_graph_creator()
    .await?
    .create_publish_graph(CreatePublishGraphOptions {
      packages: &packages,
      build_fast_check_graph,
      validate_graph: true,
      skip_unanalyzable_exports: false,
    })
    .await?;
  if !build_fast_check_graph || has_slow_types(&graph, &packages)? {
    return Ok(());
  }

  // type checks the public API output, as a temporary measure until we know
  // that it's reliable and stable
  let mut diagnostics_by_folder =
    factory.type_checker().await?.check_diagnostics(
      graph,
      CheckOptions {
        build_fast_check_graph: false, // already built
        lib: cli_options.ts_type_lib_window(),
        reload: cli_options.reload_flag(),
        type_check_mode: cli_options.type_check_mode(),
      },
    )?;
  // ignore unused (type) parameter diagnostics that may occur due to fast
  // check not having function body implementations
  for result in diagnostics_by_folder.by_ref() {
    let check_diagnostics = result?.filter(|d| d.include_when_remote());
    if check_diagnostics.has_diagnostic() {
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
  }
  Ok(())
}

fn has_slow_types(
  graph: &deno_graph::ModuleGraph,
  packages: &[JsrPackageConfig],
) -> Result<bool, AnyError> {
  for package in packages {
    let export_urls = package.config_file.resolve_export_value_urls()?;
    if !collect_no_slow_type_diagnostics(graph, &export_urls).is_empty() {
      return Ok(true);
    }
  }
  Ok(false)
}

async fn run_jsr_cli(
  factory: &CliFactory,
  args: Vec<String>,
) -> Result<i32, AnyError> {
  let bin_path = match std::env::var_os("JSR_BIN_PATH") {
    Some(path) => PathBuf::from(path),
    None => resolve_jsr_cli(factory).await?,
  };
  log::debug!("Running {} {}", bin_path.display(), args.join(" "));
  let status = tokio::process::Command::new(&bin_path)
    .args(&args)
    .status()
    .await
    .with_context(|| format!("Failed running {}", bin_path.display()))?;
  Ok(status.code().unwrap_or(1))
}

/// Returns the path of the newest jsr binary in `JSR_CLI_VERSION_REQ`,
/// downloading it into `DENO_DIR` if needed. Falls back to the newest
/// downloaded one when the version can't be resolved, like when offline.
async fn resolve_jsr_cli(factory: &CliFactory) -> Result<PathBuf, AnyError> {
  let version_req = VersionReq::parse_from_npm(JSR_CLI_VERSION_REQ).unwrap();
  let dl_dir = factory.deno_dir()?.dl_folder_path().join("jsr-cli");
  let client = factory.http_client_provider().get_or_create()?;
  let registry_url = factory.npmrc()?.get_registry_url("jsr").clone();

  let version =
    match resolve_newest_version(&client, &registry_url, &version_req).await {
      Ok(version) => version,
      Err(err) => match newest_downloaded_version(&dl_dir, &version_req) {
        Some(version) => {
          log::debug!("Using downloaded jsr CLI {version}: {err:#}");
          version
        }
        None => return Err(err.context("Failed resolving the jsr CLI")),
      },
    };

  let exe_name = if cfg!(windows) { "jsr.exe" } else { "jsr" };
  let bin_path = dl_dir.join(version.to_string()).join(exe_name);
  if !bin_path.exists() {
    download_jsr_cli(&client, &version, &bin_path)
      .await
      .with_context(|| format!("Failed downloading the jsr CLI {version}"))?;
  }
  Ok(bin_path)
}

/// Resolves the version from the `jsr` npm package, which is published for
/// every release of the jsr CLI.
async fn resolve_newest_version(
  client: &HttpClient,
  registry_url: &Url,
  version_req: &VersionReq,
) -> Result<Version, AnyError> {
  #[derive(serde::Deserialize)]
  struct PackageInfo {
    versions: serde_json::Map<String, serde_json::Value>,
  }

  let url = registry_url.join("jsr")?;
  let info: PackageInfo =
    serde_json::from_str(&client.download_text(url).await?)?;
  info
    .versions
    .keys()
    .filter_map(|version| Version::parse_from_npm(version).ok())
    .filter(|version| version_req.matches(version))
    .max()
    .with_context(|| {
      format!("No jsr CLI version matches {JSR_CLI_VERSION_REQ}")
    })
}

fn newest_downloaded_version(
  dl_dir: &Path,
  version_req: &VersionReq,
) -> Option<Version> {
  std::fs::read_dir(dl_dir)
    .ok()?
    .filter_map(|entry| entry.ok())
    .filter_map(|entry| {
      Version::parse_from_npm(entry.file_name().to_str()?).ok()
    })
    .filter(|version| version_req.matches(version))
    .max()
}

async fn download_jsr_cli(
  client: &HttpClient,
  version: &Version,
  bin_path: &Path,
) -> Result<(), AnyError> {
  let archive_name = format!("jsr-{}.zip", env!("TARGET"));
  let archive_url: Url =
    format!("{JSR_CLI_RELEASES_URL}/download/v{version}/{archive_name}")
      .parse()?;
  let checksum_url: Url = format!("{archive_url}.sha256sum").parse()?;

  let checksum = client.download_text(checksum_url).await?;
  // formatted as "<hash>  <filename>"
  let expected_hash = checksum
    .split_whitespace()
    .next()
    .unwrap_or_default()
    .to_lowercase();
  let archive_data = {
    let progress_bars = ProgressBar::new(ProgressBarStyle::DownloadBars);
    let progress = progress_bars.update(archive_url.as_str());
    client
      .download_with_progress_and_retries(
        archive_url.clone(),
        &Default::default(),
        &progress,
      )
      .await?
      .into_bytes()?
  };
  let actual_hash =
    faster_hex::hex_string(&sha2::Sha256::digest(&archive_data));
  if actual_hash != expected_hash {
    bail!(
      "Checksum mismatch for {archive_url}.\n  Expected: {expected_hash}\n  Actual:   {actual_hash}"
    );
  }

  let version_dir = bin_path.parent().unwrap();
  let dl_dir = version_dir.parent().unwrap();
  std::fs::create_dir_all(dl_dir)?;
  // unpack next to the destination, then move it into place, so that a
  // concurrent run never sees a partial binary
  let temp_dir = tempfile::TempDir::new_in(dl_dir)?;
  let exe_name = bin_path.file_name().unwrap().to_str().unwrap();
  let unpacked = archive::unpack_into_dir(archive::UnpackArgs {
    exe_name,
    archive_name: &archive_name,
    archive_data: &archive_data,
    dest_path: temp_dir.path(),
  })?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
      &unpacked,
      std::fs::Permissions::from_mode(0o755),
    )?;
  }
  std::fs::create_dir_all(version_dir)?;
  if let Err(err) = std::fs::rename(&unpacked, bin_path)
    && !bin_path.exists()
  {
    return Err(err.into());
  }
  Ok(())
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn version_req_parses() {
    VersionReq::parse_from_npm(JSR_CLI_VERSION_REQ).unwrap();
  }

  #[test]
  fn shared_args() {
    let mut args = Vec::new();
    let mut flags = Flags {
      config_flag: ConfigFlag::Path("deno.jsonc".to_string()),
      log_level: Some(log::Level::Debug),
      ..Default::default()
    };
    flags.unstable_config.sloppy_imports = true;
    push_shared_args(&mut args, &flags);
    assert_eq!(
      args,
      [
        "--config",
        "deno.jsonc",
        "--unstable-sloppy-imports",
        "--log-level=debug"
      ]
    );
  }
}
