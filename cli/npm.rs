// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::num::NonZeroUsize;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use dashmap::DashMap;
use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_lib::version::DENO_VERSION_INFO;
use deno_npm::NpmResolutionPackage;
use deno_npm::registry::NpmPackageInfo;
use deno_npm::registry::NpmPackageVersionInfosIterator;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::NpmVersionResolver;
use deno_npm_cache::NpmCacheHttpClientBytesResponse;
use deno_npm_cache::NpmCacheHttpClientResponse;
use deno_npm_installer::BinEntries;
use deno_npm_installer::CachedNpmPackageExtraInfoProvider;
use deno_npm_installer::ExpectedExtraInfo;
use deno_npm_installer::lifecycle_scripts::LIFECYCLE_SCRIPTS_RUNNING_ENV_VAR;
use deno_npm_installer::lifecycle_scripts::LifecycleScriptsExecutor;
use deno_npm_installer::lifecycle_scripts::LifecycleScriptsExecutorOptions;
use deno_npm_installer::lifecycle_scripts::PackageWithScript;
use deno_npm_installer::lifecycle_scripts::compute_lifecycle_script_layers;
use deno_npm_installer::lifecycle_scripts::is_broken_default_install_script;
use deno_npmrc::RegistryConfig;
use deno_npmrc::ResolvedNpmRc;
use deno_resolver::file_fetcher::FetchOptions;
use deno_resolver::file_fetcher::FetchPermissionsOptionRef;
use deno_resolver::npm::ByonmNpmResolverCreateOptions;
use deno_resolver::npm::ManagedNpmResolverRc;
use deno_runtime::deno_io::FromRawIoHandle;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_task_shell::KillSignal;
use sys_traits::PathsInErrorsExt;

use crate::file_fetcher::CliFileFetcher;
use crate::http_util::HttpClientProvider;
use crate::sys::CliSys;
use crate::task_runner::TaskStdio;
use crate::util::progress_bar::ProgressBar;
use crate::util::progress_bar::ProgressMessagePrompt;

pub type CliNpmInstallerFactory = deno_npm_installer::NpmInstallerFactory<
  CliNpmCacheHttpClient,
  ProgressBar,
  CliSys,
>;
pub type CliNpmInstaller =
  deno_npm_installer::NpmInstaller<CliNpmCacheHttpClient, CliSys>;
pub type CliNpmCache = deno_npm_cache::NpmCache<CliSys>;
pub type CliNpmRegistryInfoProvider =
  deno_npm_cache::RegistryInfoProvider<CliNpmCacheHttpClient, CliSys>;
pub type CliNpmResolver<TSys = CliSys> = deno_resolver::npm::NpmResolver<TSys>;
pub type CliManagedNpmResolver = deno_resolver::npm::ManagedNpmResolver<CliSys>;
pub type CliNpmResolverCreateOptions =
  deno_resolver::npm::NpmResolverCreateOptions<CliSys>;
pub type CliByonmNpmResolverCreateOptions =
  ByonmNpmResolverCreateOptions<CliSys>;
pub type CliNpmGraphResolver = deno_npm_installer::graph::NpmDenoGraphResolver<
  CliNpmCacheHttpClient,
  CliSys,
>;

pub use deno_npm_cache::NpmPackumentFormat;

/// `Accept` header sent when fetching npm package metadata. Mirrors the npm
/// install path (`CliNpmCacheHttpClient`) so registries that content-negotiate
/// (or redirect non-npm-client requests elsewhere) behave the same for metadata
/// lookups done by `deno outdated`, `deno add`, etc.
const NPM_PACKAGE_INFO_ACCEPT: &str =
  "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*";

#[derive(Debug)]
pub struct CliNpmCacheHttpClient {
  http_client_provider: Arc<HttpClientProvider>,
  progress_bar: ProgressBar,
  packument_format: NpmPackumentFormat,
}

impl CliNpmCacheHttpClient {
  pub fn new(
    http_client_provider: Arc<HttpClientProvider>,
    progress_bar: ProgressBar,
    packument_format: NpmPackumentFormat,
  ) -> Self {
    Self {
      http_client_provider,
      progress_bar,
      packument_format,
    }
  }

  fn get_or_create_http_client(
    &self,
    maybe_registry_config: Option<&RegistryConfig>,
  ) -> Result<crate::http_util::HttpClient, deno_error::JsErrorBox> {
    if let Some(config) = maybe_registry_config
      && let (Some(certfile), Some(keyfile)) =
        (&config.certfile, &config.keyfile)
    {
      return self.http_client_provider.get_or_create_with_client_cert(
        std::path::Path::new(certfile),
        std::path::Path::new(keyfile),
      );
    }
    self.http_client_provider.get_or_create()
  }
}

#[async_trait::async_trait(?Send)]
impl deno_npm_cache::NpmCacheHttpClient for CliNpmCacheHttpClient {
  async fn download_with_retries_on_any_tokio_runtime(
    &self,
    url: Url,
    maybe_auth: Option<String>,
    maybe_etag: Option<String>,
    maybe_registry_config: Option<&RegistryConfig>,
  ) -> Result<NpmCacheHttpClientResponse, deno_npm_cache::DownloadError> {
    let guard = self.progress_bar.update(url.as_str());
    let client = self
      .get_or_create_http_client(maybe_registry_config)
      .map_err(|err| deno_npm_cache::DownloadError {
        status_code: None,
        error: err,
      })?;
    let mut headers = http::HeaderMap::new();
    if let Some(auth) = maybe_auth {
      headers.append(
        http::header::AUTHORIZATION,
        http::header::HeaderValue::try_from(auth).unwrap(),
      );
    }
    if let Some(etag) = maybe_etag {
      headers.append(
        http::header::IF_NONE_MATCH,
        http::header::HeaderValue::try_from(etag).unwrap(),
      );
    }
    if self.packument_format == NpmPackumentFormat::Abbreviated {
      // Request the abbreviated install manifest when possible. This is 2-5x
      // smaller than the full packument (e.g. @types/node: 2.3 MB vs 10.9 MB).
      // Uses content negotiation with quality factors for registry compatibility
      // (some registries like older Artifactory don't support the abbreviated
      // format and need the JSON fallback).
      //
      // Not used when minimumDependencyAge is configured, because the
      // abbreviated format omits the `time` field needed for date filtering.
      headers.insert(
        http::header::ACCEPT,
        http::header::HeaderValue::from_static(
          "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*",
        ),
      );
    }
    // Request gzip and bypass the tower-http Decompression middleware so
    // that gzip inflate happens on a blocking thread instead of inline on
    // the async event loop. This prevents large packument decompression
    // (~12% of CPU during resolution) from blocking other HTTP/2 streams.
    headers.insert(
      http::header::ACCEPT_ENCODING,
      http::header::HeaderValue::from_static("gzip"),
    );
    let response = client
      .download_with_progress_and_retries_no_decompress(url, &headers, &guard)
      .await
      .map_err(|err| {
        use crate::http_util::DownloadErrorKind::*;
        let status_code = match err.as_kind() {
          Fetch { .. }
          | UrlParse { .. }
          | HttpParse { .. }
          | Json { .. }
          | ToStr { .. }
          | RedirectHeaderParse { .. }
          | TooManyRedirects
          | UnhandledNotModified
          | NotFound
          | Other(_) => None,
          BadResponse(bad_response_error) => {
            Some(bad_response_error.status_code.as_u16())
          }
        };
        deno_npm_cache::DownloadError {
          status_code,
          error: JsErrorBox::from_err(err),
        }
      })?;
    match response {
      crate::http_util::HttpClientResponse::Success { headers, body } => {
        // Decompress gzip on a blocking thread to keep the event loop free
        let body = if headers
          .get(http::header::CONTENT_ENCODING)
          .and_then(|v| v.to_str().ok())
          .is_some_and(|v| v == "gzip")
        {
          tokio::task::spawn_blocking(move || decompress_gzip(body))
            .await
            .map_err(|e| deno_npm_cache::DownloadError {
              status_code: None,
              error: JsErrorBox::generic(e.to_string()),
            })?
            .map_err(|e| deno_npm_cache::DownloadError {
              status_code: None,
              error: e,
            })?
        } else {
          body
        };
        Ok(NpmCacheHttpClientResponse::Bytes(
          NpmCacheHttpClientBytesResponse {
            etag: headers
              .get(http::header::ETAG)
              .and_then(|e| e.to_str().map(|t| t.to_string()).ok()),
            bytes: body,
          },
        ))
      }
      crate::http_util::HttpClientResponse::NotFound => {
        Ok(NpmCacheHttpClientResponse::NotFound)
      }
      crate::http_util::HttpClientResponse::NotModified => {
        Ok(NpmCacheHttpClientResponse::NotModified)
      }
    }
  }
}

fn decompress_gzip(compressed: Vec<u8>) -> Result<Vec<u8>, JsErrorBox> {
  use std::io::Read;

  use flate2::read::GzDecoder;
  let mut decoder = GzDecoder::new(compressed.as_slice());
  let mut decompressed = Vec::new();
  decoder.read_to_end(&mut decompressed).map_err(|e| {
    JsErrorBox::generic(format!("gzip decompression failed: {e}"))
  })?;
  Ok(decompressed)
}

/// Why fetching package metadata for a single package failed. Used to surface
/// actionable diagnostics (e.g. in `deno outdated`) instead of silently
/// dropping packages that live on unreachable or unauthorized registries.
#[derive(Clone, Debug)]
pub struct PackageInfoLoadError {
  /// The registry URL the metadata was being fetched from.
  pub registry_url: String,
  /// A human readable description of what went wrong.
  pub reason: String,
}

#[derive(Debug)]
pub struct NpmFetchResolver {
  nv_by_req: DashMap<PackageReq, Option<PackageNv>>,
  info_by_name:
    DashMap<String, Result<Arc<NpmPackageInfo>, Arc<PackageInfoLoadError>>>,
  file_fetcher: Arc<CliFileFetcher>,
  npmrc: Arc<ResolvedNpmRc>,
  version_resolver: Arc<NpmVersionResolver>,
}

impl NpmFetchResolver {
  pub fn new(
    file_fetcher: Arc<CliFileFetcher>,
    npmrc: Arc<ResolvedNpmRc>,
    version_resolver: Arc<NpmVersionResolver>,
  ) -> Self {
    Self {
      nv_by_req: Default::default(),
      info_by_name: Default::default(),
      file_fetcher,
      npmrc,
      version_resolver,
    }
  }

  pub async fn req_to_nv(
    &self,
    req: &PackageReq,
  ) -> Result<Option<PackageNv>, AnyError> {
    if let Some(nv) = self.nv_by_req.get(req) {
      return Ok(nv.value().clone());
    }
    let maybe_get_nv = || async {
      let name = &req.name;
      let Some(package_info) = self.package_info(name).await else {
        return Result::<Option<PackageNv>, AnyError>::Ok(None);
      };
      let version_resolver =
        self.version_resolver.get_for_package(&package_info);
      let version_info = version_resolver.resolve_best_package_version_info(
        &req.version_req,
        Vec::new().into_iter(),
      )?;
      Ok(Some(PackageNv {
        name: name.clone(),
        version: version_info.version.clone(),
      }))
    };
    let nv = maybe_get_nv().await?;
    self.nv_by_req.insert(req.clone(), nv.clone());
    Ok(nv)
  }

  pub async fn package_info(&self, name: &str) -> Option<Arc<NpmPackageInfo>> {
    self.package_info_with_reason(name).await.ok()
  }

  /// Like [`Self::package_info`], but preserves the reason the fetch failed
  /// (e.g. an HTTP 401 from a private registry) so callers can surface it
  /// instead of silently treating the package as having no available versions.
  pub async fn package_info_with_reason(
    &self,
    name: &str,
  ) -> Result<Arc<NpmPackageInfo>, Arc<PackageInfoLoadError>> {
    if let Some(info) = self.info_by_name.get(name) {
      return info.value().clone();
    }
    let registry_url = self.npmrc.get_registry_url(name).to_string();
    let result =
      self
        .fetch_package_info(name)
        .await
        .map(Arc::new)
        .map_err(|reason| {
          Arc::new(PackageInfoLoadError {
            registry_url: registry_url.clone(),
            reason,
          })
        });
    self.info_by_name.insert(name.to_string(), result.clone());
    result
  }

  // todo(#27198): use RegistryInfoProvider instead
  async fn fetch_package_info(
    &self,
    name: &str,
  ) -> Result<NpmPackageInfo, String> {
    let info_url = deno_npm_cache::get_package_url(&self.npmrc, name);
    let registry_config = self.npmrc.get_registry_config(name);
    let maybe_auth_header =
      deno_npm_cache::maybe_auth_header_value_for_npm_registry(registry_config)
        .map_err(AnyError::from)
        .and_then(|value| match value {
          Some(value) => Ok(Some((
            http::header::AUTHORIZATION,
            http::HeaderValue::try_from(value.into_bytes())?,
          ))),
          None => Ok(None),
        })
        .map_err(|e| format!("{e:#}"))?;
    let file = self
      .file_fetcher
      .fetch_with_options(
        &info_url,
        FetchPermissionsOptionRef::AllowAll,
        FetchOptions {
          maybe_auth: maybe_auth_header,
          // Identify as an npm client. Some registries (e.g. self-hosted
          // GitLab) only serve package metadata to requests that send the npm
          // `Accept` header and otherwise redirect to registry.npmjs.org, which
          // drops the auth header on the cross-origin redirect and 404s for
          // private packages. The npm install path sends this same header, so
          // matching it keeps `deno outdated`/`deno add` working wherever
          // `deno install` does. See https://github.com/denoland/deno/issues/31924
          maybe_accept: Some(NPM_PACKAGE_INFO_ACCEPT),
          ..Default::default()
        },
      )
      .await
      .map_err(|e| format!("{e:#}"))?;
    serde_json::from_slice::<NpmPackageInfo>(&file.source)
      .map_err(|e| format!("failed to parse package metadata: {e}"))
  }

  pub fn applicable_version_infos<'a>(
    &'a self,
    package_info: &'a NpmPackageInfo,
  ) -> NpmPackageVersionInfosIterator<'a> {
    self
      .version_resolver
      .get_for_package(package_info)
      .applicable_version_infos()
  }
}

pub static NPM_CONFIG_USER_AGENT_ENV_VAR: &str = "npm_config_user_agent";

pub fn get_npm_config_user_agent() -> String {
  format!(
    "deno/{} npm/? deno/{} {} {}",
    DENO_VERSION_INFO.deno,
    DENO_VERSION_INFO.deno,
    std::env::consts::OS,
    std::env::consts::ARCH
  )
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum DenoTaskLifecycleScriptsError {
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[class(inherit)]
  #[error(
    "failed to create npm process state tempfile for running lifecycle scripts"
  )]
  CreateNpmProcessState(#[source] std::io::Error),
  #[class(generic)]
  #[error(transparent)]
  Task(AnyError),
  #[class(generic)]
  #[error("failed to run scripts for packages: {}", .0.join(", "))]
  RunScripts(Vec<String>),
}

pub struct DenoTaskLifeCycleScriptsExecutor {
  progress_bar: ProgressBar,
  npm_resolver: ManagedNpmResolverRc<CliSys>,
  system_info: deno_npm::NpmSystemInfo,
}

struct PackageScriptResult<'a> {
  package: &'a NpmResolutionPackage,
  failed: Option<&'a PackageNv>,
}

#[async_trait::async_trait(?Send)]
impl LifecycleScriptsExecutor for DenoTaskLifeCycleScriptsExecutor {
  async fn execute(
    &self,
    options: LifecycleScriptsExecutorOptions<'_>,
  ) -> Result<(), AnyError> {
    let mut failed_packages = Vec::new();
    let sys = CliSys::default();
    let mut bin_entries = BinEntries::new(sys.with_paths_in_errors());
    // get custom commands for each bin available in the node_modules dir (essentially
    // the scripts that are in `node_modules/.bin`)
    let base = self
      .resolve_baseline_custom_commands(
        options.extra_info_provider,
        &mut bin_entries,
        options.snapshot,
        options.system_packages,
      )
      .await;

    // we don't run with signals forwarded because once signals
    // are setup then they're process wide.
    let kill_signal = KillSignal::default();
    let _drop_signal = kill_signal.clone().drop_guard();

    let mut env_vars = crate::task_runner::real_env_vars();
    // so the subprocess can detect that it is running as part of a lifecycle script,
    // and avoid trying to set up node_modules again
    env_vars.insert(LIFECYCLE_SCRIPTS_RUNNING_ENV_VAR.into(), "1".into());

    let concurrency = std::thread::available_parallelism()
      .ok()
      .and_then(|n| NonZeroUsize::new(n.get().saturating_sub(1)))
      .unwrap_or_else(|| NonZeroUsize::new(2).unwrap())
      .get();

    let layers = compute_lifecycle_script_layers(
      options.packages_with_scripts,
      options.snapshot,
      options.additional_packages,
    );

    for layer in &layers {
      log::debug!(
        "Running lifecycle scripts layer: {}",
        layer
          .iter()
          .map(|l| l.package.id.as_serialized())
          .collect::<Vec<_>>()
          .join(", ")
      );

      let mut results =
        deno_core::futures::stream::iter(layer.iter().map(|pkg| {
          self.run_single_package_scripts(
            pkg,
            &env_vars,
            &base,
            &options,
            &kill_signal,
            &sys,
          )
        }))
        .buffer_unordered(concurrency);

      while let Some(result) = results.next().await {
        let result = result?;
        if let Some(nv) = result.failed {
          failed_packages.push(nv);
        }
        (options.on_ran_pkg_scripts)(result.package)?;
      }
    }

    // re-set up bin entries for the packages which we've run scripts for.
    // lifecycle scripts can create files that are linked to by bin entries,
    // and the only reliable way to handle this is to re-link bin entries
    // (this is what PNPM does as well)
    let package_ids = options
      .packages_with_scripts
      .iter()
      .map(|p| &p.package.id)
      .collect::<HashSet<_>>();
    bin_entries.finish_only(
      options.snapshot,
      &options.root_node_modules_dir_path.join(".bin"),
      |outcome| outcome.warn_if_failed(),
      &package_ids,
    )?;

    if failed_packages.is_empty() {
      Ok(())
    } else {
      Err(
        DenoTaskLifecycleScriptsError::RunScripts(
          failed_packages
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>(),
        )
        .into(),
      )
    }
  }
}

impl DenoTaskLifeCycleScriptsExecutor {
  pub fn new(
    npm_resolver: ManagedNpmResolverRc<CliSys>,
    progress_bar: ProgressBar,
    system_info: deno_npm::NpmSystemInfo,
  ) -> Self {
    Self {
      npm_resolver,
      progress_bar,
      system_info,
    }
  }

  /// Runs lifecycle scripts for a single package (preinstall, install,
  /// postinstall in order). Each package gets its own temp file for
  /// npm process state so concurrent execution is safe.
  async fn run_single_package_scripts<'a>(
    &self,
    pkg: &'a PackageWithScript<'a>,
    env_vars: &HashMap<OsString, OsString>,
    base_custom_commands: &crate::task_runner::TaskCustomCommands,
    options: &LifecycleScriptsExecutorOptions<'a>,
    kill_signal: &KillSignal,
    sys: &CliSys,
  ) -> Result<PackageScriptResult<'a>, AnyError> {
    let PackageWithScript {
      package,
      scripts,
      package_folder,
      init_cwds,
    } = pkg;
    // Run the scripts once per workspace member that depends on this package,
    // with `INIT_CWD` pointing at that member, so workspace-aware scripts (e.g.
    // `@sveltejs/kit`'s `postinstall`) operate on each member. When no member
    // declares the package directly, run once with the global init cwd.
    let init_cwds: Vec<&Path> = if init_cwds.is_empty() {
      vec![options.init_cwd]
    } else {
      init_cwds.iter().map(|p| p.as_path()).collect()
    };

    // each concurrent package gets its own temp file to avoid fd races
    let temp_file_fd = deno_runtime::deno_process::npm_process_state_tempfile(
      options.process_state.as_bytes(),
    )
    .map_err(DenoTaskLifecycleScriptsError::CreateNpmProcessState)?;
    // SAFETY: fd/handle is valid
    let _temp_file = unsafe { std::fs::File::from_raw_io_handle(temp_file_fd) };
    let mut env_vars = env_vars.clone();
    env_vars.insert(
      deno_runtime::deno_process::NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME.into(),
      (temp_file_fd as usize).to_string().into(),
    );

    // add custom commands for binaries from the package's dependencies.
    // this will take precedence over the baseline commands, so if the
    // package relies on a bin that conflicts with one higher in the
    // dependency tree, the correct bin will be used.
    let custom_commands = self
      .resolve_custom_commands_from_deps(
        options.extra_info_provider,
        base_custom_commands.clone(),
        package,
        options.snapshot,
        options.additional_packages,
      )
      .await;

    let mut failed = None;
    'cwds: for init_cwd in init_cwds {
      for script_name in ["preinstall", "install", "postinstall"] {
        if let Some(script) = scripts.get(script_name) {
          if script_name == "install"
            && is_broken_default_install_script(sys, script, package_folder)
          {
            continue;
          }
          let _guard = self.progress_bar.update_with_prompt(
            ProgressMessagePrompt::Initialize,
            &format!("{}: running '{script_name}' script", package.id.nv),
          );
          let crate::task_runner::TaskResult {
            exit_code,
            stderr,
            stdout,
          } =
            crate::task_runner::run_task(crate::task_runner::RunTaskOptions {
              task_name: script_name,
              script,
              cwd: package_folder.clone(),
              env_vars: env_vars.clone(),
              custom_commands: custom_commands.clone(),
              init_cwd,
              argv: &[],
              node_modules_bin_dirs: &[options
                .root_node_modules_dir_path
                .join(".bin")],
              stdio: Some(crate::task_runner::TaskIo {
                stderr: TaskStdio::piped(),
                stdout: TaskStdio::piped(),
              }),
              kill_signal: kill_signal.clone(),
            })
            .await
            .map_err(DenoTaskLifecycleScriptsError::Task)?;
          let stdout = stdout.unwrap();
          let stderr = stderr.unwrap();
          if exit_code != 0 {
            log::warn!(
              "error: script '{}' in '{}' failed with exit code {}{}{}",
              script_name,
              package.id.nv,
              exit_code,
              if !stdout.trim_ascii().is_empty() {
                format!(
                  "\nstdout:\n{}\n",
                  String::from_utf8_lossy(&stdout).trim()
                )
              } else {
                String::new()
              },
              if !stderr.trim_ascii().is_empty() {
                format!(
                  "\nstderr:\n{}\n",
                  String::from_utf8_lossy(&stderr).trim()
                )
              } else {
                String::new()
              },
            );
            failed = Some(&package.id.nv);
            // assume if earlier script fails, later ones will fail too
            break 'cwds;
          }
        }
      }
    }

    Ok(PackageScriptResult { package, failed })
  }

  // take in all (non copy) packages from snapshot,
  // and resolve the set of available binaries to create
  // custom commands available to the task runner
  async fn resolve_baseline_custom_commands<'a>(
    &self,
    extra_info_provider: &CachedNpmPackageExtraInfoProvider,
    bin_entries: &mut BinEntries<'a, CliSys>,
    snapshot: &'a NpmResolutionSnapshot,
    packages: &'a [NpmResolutionPackage],
  ) -> crate::task_runner::TaskCustomCommands {
    let mut custom_commands = crate::task_runner::TaskCustomCommands::new();
    custom_commands
      .insert("npx".to_string(), Rc::new(crate::task_runner::NpxCommand));

    custom_commands
      .insert("npm".to_string(), Rc::new(crate::task_runner::NpmCommand));

    custom_commands
      .insert("node".to_string(), Rc::new(crate::task_runner::NodeCommand));

    custom_commands.insert(
      "node-gyp".to_string(),
      Rc::new(crate::task_runner::NodeGypCommand),
    );

    // TODO: this recreates the bin entries which could be redoing some work, but the ones
    // we compute earlier in `sync_resolution_with_fs` may not be exhaustive (because we skip
    // doing it for packages that are set up already.
    // realistically, scripts won't be run very often so it probably isn't too big of an issue.
    self
      .resolve_custom_commands_from_packages(
        extra_info_provider,
        bin_entries,
        custom_commands,
        snapshot,
        packages,
      )
      .await
  }

  // resolves the custom commands from an iterator of packages
  // and adds them to the existing custom commands.
  // note that this will overwrite any existing custom commands
  async fn resolve_custom_commands_from_packages<
    'a,
    P: IntoIterator<Item = &'a NpmResolutionPackage>,
  >(
    &self,
    extra_info_provider: &CachedNpmPackageExtraInfoProvider,
    bin_entries: &mut BinEntries<'a, CliSys>,
    mut commands: crate::task_runner::TaskCustomCommands,
    snapshot: &'a NpmResolutionSnapshot,
    packages: P,
  ) -> crate::task_runner::TaskCustomCommands {
    for package in packages {
      let Ok(package_path) = self
        .npm_resolver
        .resolve_pkg_folder_from_pkg_id(&package.id)
      else {
        continue;
      };
      let extra = if let Some(extra) = &package.extra {
        Cow::Borrowed(extra)
      } else {
        let Ok(extra) = extra_info_provider
          .get_package_extra_info(
            &package.id.nv,
            &package_path,
            ExpectedExtraInfo::from_package(package),
          )
          .await
        else {
          continue;
        };
        Cow::Owned(extra)
      };
      if extra.bin.is_some() {
        bin_entries.add(package, &extra, package_path);
      }
    }

    let bins: Vec<(String, PathBuf)> = bin_entries.collect_bin_files(snapshot);
    for (bin_name, script_path) in bins {
      commands.insert(
        bin_name.clone(),
        Rc::new(crate::task_runner::NodeModulesFileRunCommand {
          command_name: bin_name,
          path: script_path,
        }),
      );
    }

    commands
  }

  // resolves the custom commands from the dependencies of a package
  // and adds them to the existing custom commands.
  // note that this will overwrite any existing custom commands.
  async fn resolve_custom_commands_from_deps(
    &self,
    extra_info_provider: &CachedNpmPackageExtraInfoProvider,
    baseline: crate::task_runner::TaskCustomCommands,
    package: &NpmResolutionPackage,
    snapshot: &NpmResolutionSnapshot,
    additional_packages: &[&NpmResolutionPackage],
  ) -> crate::task_runner::TaskCustomCommands {
    let sys = CliSys::default();
    let mut bin_entries = BinEntries::new(sys.with_paths_in_errors());
    self
      .resolve_custom_commands_from_packages(
        extra_info_provider,
        &mut bin_entries,
        baseline,
        snapshot,
        package.dependencies.iter().filter_map(|(name, id)| {
          let dep = snapshot.package_from_id(id).or_else(|| {
            additional_packages
              .iter()
              .find(|package| package.id == *id)
              .copied()
          })?;
          // Skip optional dependencies that don't match the current system
          if package.optional_dependencies.contains(name)
            && !dep.system.matches_system(&self.system_info)
          {
            return None;
          }
          Some(dep)
        }),
      )
      .await
  }
}
