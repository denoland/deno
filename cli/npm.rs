// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use dashmap::DashMap;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_lib::version::DENO_VERSION_INFO;
use deno_npm::NpmResolutionPackage;
use deno_npm::npm_rc::ResolvedNpmRc;
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
use deno_npm_installer::lifecycle_scripts::is_broken_default_install_script;
use deno_resolver::npm::ByonmNpmResolverCreateOptions;
use deno_resolver::npm::ManagedNpmResolverRc;
use deno_runtime::deno_io::FromRawIoHandle;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_task_shell::KillSignal;

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
pub type CliNpmResolver = deno_resolver::npm::NpmResolver<CliSys>;
pub type CliManagedNpmResolver = deno_resolver::npm::ManagedNpmResolver<CliSys>;
pub type CliNpmResolverCreateOptions =
  deno_resolver::npm::NpmResolverCreateOptions<CliSys>;
pub type CliByonmNpmResolverCreateOptions =
  ByonmNpmResolverCreateOptions<CliSys>;
pub type CliNpmGraphResolver = deno_npm_installer::graph::NpmDenoGraphResolver<
  CliNpmCacheHttpClient,
  CliSys,
>;

#[derive(Debug)]
pub struct CliNpmCacheHttpClient {
  http_client_provider: Arc<HttpClientProvider>,
  progress_bar: ProgressBar,
}

impl CliNpmCacheHttpClient {
  pub fn new(
    http_client_provider: Arc<HttpClientProvider>,
    progress_bar: ProgressBar,
  ) -> Self {
    Self {
      http_client_provider,
      progress_bar,
    }
  }
}

#[async_trait::async_trait(?Send)]
impl deno_npm_cache::NpmCacheHttpClient for CliNpmCacheHttpClient {
  async fn download_with_retries_on_any_tokio_runtime(
    &self,
    url: Url,
    maybe_auth: Option<String>,
    maybe_etag: Option<String>,
  ) -> Result<NpmCacheHttpClientResponse, deno_npm_cache::DownloadError> {
    let guard = self.progress_bar.update(url.as_str());
    let client = self.http_client_provider.get_or_create().map_err(|err| {
      deno_npm_cache::DownloadError {
        status_code: None,
        error: err,
      }
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
    client
      .download_with_progress_and_retries(url, &headers, &guard)
      .await
      .map(|response| match response {
        crate::http_util::HttpClientResponse::Success { headers, body } => {
          NpmCacheHttpClientResponse::Bytes(NpmCacheHttpClientBytesResponse {
            etag: headers
              .get(http::header::ETAG)
              .and_then(|e| e.to_str().map(|t| t.to_string()).ok()),
            bytes: body,
          })
        }
        crate::http_util::HttpClientResponse::NotFound => {
          NpmCacheHttpClientResponse::NotFound
        }
        crate::http_util::HttpClientResponse::NotModified => {
          NpmCacheHttpClientResponse::NotModified
        }
      })
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
      })
  }
}

#[derive(Debug)]
pub struct NpmFetchResolver {
  nv_by_req: DashMap<PackageReq, Option<PackageNv>>,
  info_by_name: DashMap<String, Option<Arc<NpmPackageInfo>>>,
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
    if let Some(info) = self.info_by_name.get(name) {
      return info.value().clone();
    }
    // todo(#27198): use RegistryInfoProvider instead
    let fetch_package_info = || async {
      let info_url = deno_npm_cache::get_package_url(&self.npmrc, name);
      let registry_config = self.npmrc.get_registry_config(name);
      // TODO(bartlomieju): this should error out, not use `.ok()`.
      let maybe_auth_header =
        deno_npm_cache::maybe_auth_header_value_for_npm_registry(
          registry_config,
        )
        .map_err(AnyError::from)
        .and_then(|value| match value {
          Some(value) => Ok(Some((
            http::header::AUTHORIZATION,
            http::HeaderValue::try_from(value.into_bytes())?,
          ))),
          None => Ok(None),
        })
        .ok()?;
      let file = self
        .file_fetcher
        .fetch_bypass_permissions_with_maybe_auth(&info_url, maybe_auth_header)
        .await
        .ok()?;
      serde_json::from_slice::<NpmPackageInfo>(&file.source).ok()
    };
    let info = fetch_package_info().await.map(Arc::new);
    self.info_by_name.insert(name.to_string(), info.clone());
    info
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
  #[error(transparent)]
  BinEntries(#[from] deno_npm_installer::BinEntriesError),
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
}

#[async_trait::async_trait(?Send)]
impl LifecycleScriptsExecutor for DenoTaskLifeCycleScriptsExecutor {
  async fn execute(
    &self,
    options: LifecycleScriptsExecutorOptions<'_>,
  ) -> Result<(), AnyError> {
    let mut failed_packages = Vec::new();
    let sys = CliSys::default();
    let mut bin_entries = BinEntries::new(&sys);
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
    // we want to pass the current state of npm resolution down to the deno subprocess
    // (that may be running as part of the script). we do this with an inherited temp file
    //
    // SAFETY: we are sharing a single temp file across all of the scripts. the file position
    // will be shared among these, which is okay since we run only one script at a time.
    // However, if we concurrently run scripts in the future we will
    // have to have multiple temp files.
    let temp_file_fd = deno_runtime::deno_process::npm_process_state_tempfile(
      options.process_state.as_bytes(),
    )
    .map_err(DenoTaskLifecycleScriptsError::CreateNpmProcessState)?;
    // SAFETY: fd/handle is valid
    let _temp_file = unsafe { std::fs::File::from_raw_io_handle(temp_file_fd) }; // make sure the file gets closed
    env_vars.insert(
      deno_runtime::deno_process::NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME.into(),
      (temp_file_fd as usize).to_string().into(),
    );
    for PackageWithScript {
      package,
      scripts,
      package_folder,
    } in options.packages_with_scripts
    {
      // add custom commands for binaries from the package's dependencies. this will take precedence over the
      // baseline commands, so if the package relies on a bin that conflicts with one higher in the dependency tree, the
      // correct bin will be used.
      let custom_commands = self
        .resolve_custom_commands_from_deps(
          options.extra_info_provider,
          base.clone(),
          package,
          options.snapshot,
        )
        .await;
      for script_name in ["preinstall", "install", "postinstall"] {
        if let Some(script) = scripts.get(script_name) {
          if script_name == "install"
            && is_broken_default_install_script(&sys, script, package_folder)
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
              init_cwd: options.init_cwd,
              argv: &[],
              root_node_modules_dir: Some(options.root_node_modules_dir_path),
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
            failed_packages.push(&package.id.nv);
            // assume if earlier script fails, later ones will fail too
            break;
          }
        }
      }
      (options.on_ran_pkg_scripts)(package)?;
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
  ) -> Self {
    Self {
      npm_resolver,
      progress_bar,
    }
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
  ) -> crate::task_runner::TaskCustomCommands {
    let sys = CliSys::default();
    let mut bin_entries = BinEntries::new(&sys);
    self
      .resolve_custom_commands_from_packages(
        extra_info_provider,
        &mut bin_entries,
        baseline,
        snapshot,
        package
          .dependencies
          .values()
          .map(|id| snapshot.package_from_id(id).unwrap()),
      )
      .await
  }
}
