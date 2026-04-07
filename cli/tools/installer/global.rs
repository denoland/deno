// Copyright 2018-2026 the Deno authors. MIT license.

use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::ErrorKind;
use std::io::Write;
#[cfg(not(windows))]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_cache_dir::file_fetcher::CacheSetting;
use deno_config::deno_json::NodeModulesDirMode;
use deno_core::anyhow::Context;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_npm_installer::PackagesAllowedScripts;
use deno_path_util::resolve_url_or_path;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use jsonc_parser::cst::CstInputValue;
use log::Level;
use once_cell::sync::Lazy;
use regex::Regex;
use regex::RegexBuilder;

use super::bin_name_resolver::BinNameResolver;
use crate::args::CompileFlags;
use crate::args::ConfigFlag;
use crate::args::DenoSubcommand;
use crate::args::Flags;
use crate::args::InstallEntrypointsFlags;
use crate::args::InstallFlags;
use crate::args::InstallFlagsGlobal;
use crate::args::InstallFlagsLocal;
use crate::args::TypeCheckMode;
use crate::args::UninstallFlags;
use crate::args::UninstallKind;
use crate::args::resolve_no_prompt;
use crate::factory::CliFactory;
use crate::file_fetcher::CliFileFetcher;
use crate::file_fetcher::CreateCliFileFetcherOptions;
use crate::file_fetcher::create_cli_file_fetcher;
use crate::jsr::JsrFetchResolver;
use crate::npm::NpmFetchResolver;
use crate::util::env::resolve_cwd;
use crate::util::fs::canonicalize_path_maybe_not_exists;

pub async fn install_global(
  flags: Arc<Flags>,
  install_flags_global: InstallFlagsGlobal,
) -> Result<(), AnyError> {
  // ensure the module is cached
  let factory = CliFactory::from_flags(flags.clone());

  let cli_options = factory.cli_options()?;
  let http_client = factory.http_client_provider();
  let deps_http_cache = factory.global_http_cache()?;
  let create_deps_file_fetcher = |download_log_level: log::Level| {
    Arc::new(create_cli_file_fetcher(
      Default::default(),
      deno_cache_dir::GlobalOrLocalHttpCache::Global(deps_http_cache.clone()),
      http_client.clone(),
      factory.memory_files().clone(),
      factory.sys(),
      CreateCliFileFetcherOptions {
        allow_remote: true,
        cache_setting: CacheSetting::ReloadAll,
        download_log_level,
        progress_bar: None,
      },
    ))
  };

  let npmrc = factory.npmrc()?;

  let deps_file_fetcher = create_deps_file_fetcher(log::Level::Trace);
  let jsr_resolver = Arc::new(JsrFetchResolver::new(
    deps_file_fetcher.clone(),
    factory.jsr_version_resolver()?.clone(),
  ));
  let npm_resolver = Arc::new(NpmFetchResolver::new(
    deps_file_fetcher,
    npmrc.clone(),
    factory.npm_version_resolver()?.clone(),
  ));

  if matches!(flags.config_flag, ConfigFlag::Discover)
    && cli_options.workspace().deno_jsons().next().is_some()
  {
    log::warn!(
      "{} discovered config file will be ignored in the installed command. Use the --config flag if you wish to include it.",
      crate::colors::yellow("Warning")
    );
  }

  if install_flags_global.compile {
    return Box::pin(install_global_compiled(flags, install_flags_global))
      .await;
  }

  for (i, module_url) in install_flags_global.module_urls.iter().enumerate() {
    let entry_text = module_url;
    if !cli_options.initial_cwd().join(entry_text).exists() {
      // provide a helpful error message for users migrating from Deno < 3.0
      if i == 1
        && install_flags_global.args.is_empty()
        && Url::parse(entry_text).is_err()
      {
        bail!(
          concat!(
            "{} is missing a prefix. Deno 3.0 requires `--` before script arguments in `deno install -g`. ",
            "Did you mean `deno install -g {} -- {}`? Or maybe provide a `jsr:` or `npm:` prefix?",
          ),
          entry_text,
          &install_flags_global.module_urls[0],
          install_flags_global.module_urls[1..].join(" "),
        )
      }
      // check for package requirement missing prefix
      if let Ok(Err(package_req)) =
        crate::tools::pm::AddRmPackageReq::parse(entry_text, None)
      {
        if package_req.name.starts_with("@")
          && jsr_resolver
            .req_to_nv(&package_req)
            .await
            .ok()
            .flatten()
            .is_some()
        {
          bail!(
            "{entry_text} is missing a prefix. Did you mean `{}`?",
            crate::colors::yellow(format!("deno install -g jsr:{package_req}"))
          );
        } else if npm_resolver
          .req_to_nv(&package_req)
          .await
          .ok()
          .flatten()
          .is_some()
        {
          bail!(
            "{entry_text} is missing a prefix. Did you mean `{}`?",
            crate::colors::yellow(format!("deno install -g npm:{package_req}"))
          );
        }
      }
    }

    let (name_and_url, extra_bin_entries) = BinaryNameAndUrl::resolve(
      &factory.bin_name_resolver()?,
      cli_options.initial_cwd(),
      module_url,
      &install_flags_global,
    )
    .await?;

    // set up config dir
    let installation_dir = get_installer_bin_dir(
      cli_options.initial_cwd(),
      install_flags_global.root.as_deref(),
    )?;
    let npm_package_info_provider = factory
      .npm_installer_factory()?
      .lockfile_npm_package_info_provider()?;
    let deps_file_fetcher = create_deps_file_fetcher(Level::Info);
    let jsr_lockfile_fetcher = JsrLockfileFetcher {
      jsr_resolver: Arc::new(JsrFetchResolver::new(
        deps_file_fetcher.clone(),
        factory.jsr_version_resolver()?.clone(),
      )),
      file_fetcher: deps_file_fetcher,
      npmrc: npmrc.clone(),
      npm_package_info_provider,
    };
    setup_config_dir(
      &name_and_url,
      &flags,
      &installation_dir,
      Some(&jsr_lockfile_fetcher),
    )
    .await?;

    // create the install shim for the primary entry
    create_install_shim(
      &name_and_url,
      cli_options.initial_cwd(),
      &flags,
      &install_flags_global,
    )?;

    // create install shims for extra bin entries
    let mut installed_extra: Vec<&str> = Vec::new();
    let mut extra_install_err = None;
    for extra_entry in &extra_bin_entries {
      match create_install_shim(
        extra_entry,
        cli_options.initial_cwd(),
        &flags,
        &install_flags_global,
      ) {
        Ok(()) => installed_extra.push(&extra_entry.name),
        Err(err) => {
          extra_install_err = Some(err);
          break;
        }
      }
    }
    if let Some(err) = extra_install_err {
      // rollback: remove the primary shim and any extra shims that were created
      let _ = remove_shim_files(&installation_dir, &name_and_url.name);
      for extra_name in &installed_extra {
        let _ = remove_shim_files(&installation_dir, extra_name);
      }
      return Err(err);
    }

    // store extra bin entry names in the config dir for uninstall
    if !extra_bin_entries.is_empty() {
      let config_dir = installation_dir.join(format!(".{}", name_and_url.name));
      let extra_names: Vec<&str> =
        extra_bin_entries.iter().map(|e| e.name.as_str()).collect();
      fs::write(
        config_dir.join("extra_bin_entries.json"),
        serde_json::to_string(&extra_names)?,
      )?;
    }
  }
  Ok(())
}

pub async fn uninstall(
  flags: Arc<Flags>,
  uninstall_flags: UninstallFlags,
) -> Result<(), AnyError> {
  let uninstall_flags = match uninstall_flags.kind {
    UninstallKind::Global(flags) => flags,
    UninstallKind::Local(remove_flags) => {
      return crate::tools::pm::remove(flags, remove_flags).await;
    }
  };

  let cwd = resolve_cwd(flags.initial_cwd.as_deref())?;
  let installation_dir =
    get_installer_bin_dir(&cwd, uninstall_flags.root.as_deref())?;

  // ensure directory exists
  if let Ok(metadata) = fs::metadata(&installation_dir)
    && !metadata.is_dir()
  {
    return Err(anyhow!("Installation path is not a directory"));
  }

  let file_path = installation_dir.join(&uninstall_flags.name);

  let mut removed = remove_file_if_exists(&file_path)?;

  if cfg!(windows) {
    removed |= remove_file_if_exists(&file_path.with_extension("cmd"))?;
    removed |= remove_file_if_exists(&file_path.with_extension("exe"))?;
  }

  if !removed {
    return Err(anyhow!(
      "No installation found for {}",
      uninstall_flags.name
    ));
  }

  // There might be some extra files to delete
  // Note: tsconfig.json is legacy. We renamed it to deno.json in January 2023.
  // Note: deno.json and lock.json files were removed Feb 2026 in favor of a sub directory
  // Use the base file path (without extension) to compute related files
  let base_file = installation_dir.join(&uninstall_flags.name);
  for ext in ["tsconfig.json", "deno.json", "lock.json"] {
    // remove the plain extension files (e.g., name.deno.json, name.lock.json)
    let file_path_ext = base_file.with_extension(ext);
    remove_file_if_exists(&file_path_ext)?;

    // also remove the hidden per-command copies created at install time
    // (e.g., .name.deno.json)
    let hidden_file = get_hidden_file_with_ext(&base_file, ext);
    remove_file_if_exists(&hidden_file)?;

    // On Windows, installs use a shim with a .cmd extension, which means the
    // hidden files might be named like `.name.cmd.deno.json`. Attempt to remove
    // those as well to be thorough.
    #[cfg(windows)]
    {
      let base_with_cmd = base_file.with_extension("cmd");
      let hidden_cmd_file = get_hidden_file_with_ext(&base_with_cmd, ext);
      remove_file_if_exists(&hidden_cmd_file)?;
    }
  }

  // remove the .<name>/ config directory if it exists
  let config_dir = installation_dir.join(format!(".{}", uninstall_flags.name));

  // check for extra bin entries stored during install and remove their shims
  let extra_bins_file = config_dir.join("extra_bin_entries.json");
  if extra_bins_file.is_file()
    && let Ok(content) = fs::read_to_string(&extra_bins_file)
    && let Ok(extra_names) = serde_json::from_str::<Vec<String>>(&content)
  {
    for extra_name in &extra_names {
      remove_shim_files(&installation_dir, extra_name)?;
    }
  }

  if config_dir.is_dir() {
    fs::remove_dir_all(&config_dir).with_context(|| {
      format!("Failed removing directory: {}", config_dir.display())
    })?;
    log::info!("deleted {}", config_dir.display());
  }

  log::info!("✅ Successfully uninstalled {}", uninstall_flags.name);
  Ok(())
}

async fn install_global_compiled(
  flags: Arc<Flags>,
  install_flags_global: InstallFlagsGlobal,
) -> Result<(), AnyError> {
  let cwd = resolve_cwd(flags.initial_cwd.as_deref())?;
  let install_dir =
    get_installer_bin_dir(&cwd, install_flags_global.root.as_deref())?;

  if let Ok(metadata) = fs::metadata(&install_dir) {
    if !metadata.is_dir() {
      return Err(anyhow!("Installation path is not a directory"));
    }
  } else {
    fs::create_dir_all(&install_dir)?;
  }

  let source_file = install_flags_global
    .module_urls
    .first()
    .ok_or_else(|| anyhow!("No module URL provided"))?
    .clone();

  // Determine the output path
  let output = if let Some(ref name) = install_flags_global.name {
    let mut output_path = install_dir.join(name);
    if cfg!(windows) {
      output_path = output_path.with_extension("exe");
    }
    output_path.to_string_lossy().into_owned()
  } else {
    format!("{}/", install_dir.to_string_lossy())
  };

  let output_path = PathBuf::from(&output);
  if output_path.is_file() {
    if !install_flags_global.force {
      return Err(anyhow!(
        "Existing installation found. Aborting (Use -f to overwrite).",
      ));
    }
    // Remove the existing file so that the compile step doesn't
    // fail its own safety check (which guards against overwriting
    // files not produced by `deno compile`).
    std::fs::remove_file(&output_path).with_context(|| {
      format!(
        concat!(
          "Failed to remove existing installation at '{0}'.\n\n",
          "This may be because an existing {1} process is running. Please ensure ",
          "there are no running {1} processes (ex. run `pkill {1}` on Unix or ",
          "`Stop-Process -Name {1}` on Windows), and ensure you have sufficient ",
          "permission to write to the installation path."
        ),
        output_path.display(),
        output_path.file_name().map(|s| s.to_string_lossy()).unwrap_or("<unknown>".into())
      )
    })?;
  }

  let compile_flags = CompileFlags {
    source_file,
    output: Some(output.clone()),
    args: install_flags_global.args,
    target: None,
    no_terminal: false,
    icon: None,
    include: vec![],
    exclude: vec![],
    eszip: false,
    self_extracting: false,
  };

  let mut new_flags = flags.as_ref().clone();
  new_flags.subcommand = DenoSubcommand::Compile(compile_flags.clone());

  crate::tools::compile::compile(new_flags, compile_flags).await?;

  log::info!("Successfully installed {}", output);

  if !is_in_path(&install_dir) {
    let installation_dir_str = install_dir.to_string_lossy();
    log::info!("Add {} to PATH", installation_dir_str);
    if cfg!(windows) {
      log::info!("    set PATH=%PATH%;{}", installation_dir_str);
    } else {
      log::info!("    export PATH=\"{}:$PATH\"", installation_dir_str);
    }
  }

  Ok(())
}

async fn setup_config_dir(
  bin_name_and_url: &BinaryNameAndUrl,
  flags: &Flags,
  installation_dir: &Path,
  jsr_lockfile_fetcher: Option<&JsrLockfileFetcher<'_>>,
) -> Result<(), AnyError> {
  fn resolve_implicit_node_modules_dir(
    flags: &Flags,
    module_url: &Url,
  ) -> Option<NodeModulesDirMode> {
    // npm: specifier always implies manual
    if module_url.scheme() == "npm" {
      return Some(NodeModulesDirMode::Manual);
    }

    // --allow-scripts implies manual
    if !matches!(flags.allow_scripts, PackagesAllowedScripts::None) {
      return Some(NodeModulesDirMode::Manual);
    }

    None
  }

  let dir = installation_dir.join(format!(".{}", bin_name_and_url.name));
  fs::create_dir_all(&dir)
    .with_context(|| format!("failed creating '{}'", dir.display()))?;

  let config_text = if let ConfigFlag::Path(config_path) = &flags.config_flag {
    fs::read_to_string(config_path)
      .with_context(|| format!("error reading {config_path}"))?
  } else {
    "{}\n".to_string()
  };
  let config =
    jsonc_parser::cst::CstRootNode::parse(&config_text, &Default::default())?;
  let config_obj = config.object_value_or_set();
  // always remove the import map field because when someone specifies `--import-map` we
  // don't want that file to be attempted to be loaded and when they don't specify that
  // (which is just something we haven't implemented yet)
  if let Some(prop) = config_obj.get("importMap") {
    prop.remove();
    if flags.import_map_path.is_none() {
      log::warn!(
        "{} \"importMap\" field in the specified config file we be ignored. Use the --import-map flag instead.",
        crate::colors::yellow("Warning"),
      );
    }
  }
  if let Some(prop) = config_obj.get("workspace") {
    prop.remove();
    log::warn!(
      "{} \"workspace\" field in the specified config file will be ignored.",
      crate::colors::yellow("Warning"),
    );
  }
  config_obj.append("workspace", CstInputValue::Array(Vec::new())); // stop workspace discovery
  if config_obj.get("nodeModulesDir").is_none()
    && let Some(mode) =
      resolve_implicit_node_modules_dir(flags, &bin_name_and_url.module_url)
  {
    config_obj.append(
      "nodeModulesDir",
      CstInputValue::String(mode.as_str().to_string()),
    );
  }
  fs::write(dir.join("deno.json"), config.to_string())?;

  // write package.json for npm specifiers
  if let Ok(pkg_ref) =
    NpmPackageReqReference::from_specifier(&bin_name_and_url.module_url)
  {
    let req = pkg_ref.req();
    fs::write(
      dir.join("package.json"),
      format!(
        "{{\"dependencies\": {{\"{}\": \"{}\"}}}}",
        req.name, req.version_req
      ),
    )?;
  }

  // fetch deno.lock from JSR if this is a JSR package
  if !flags.no_lock
    && let Some(fetcher) = jsr_lockfile_fetcher
    && let Some(lockfile_content) =
      fetcher.fetch_lockfile(&bin_name_and_url.module_url).await
  {
    fs::write(dir.join("deno.lock"), lockfile_content)?;
  }

  // create cloned flags to run cache_top_level_deps
  let mut new_flags = flags.clone();
  new_flags.initial_cwd = Some(dir.clone());
  new_flags.node_modules_dir = flags.node_modules_dir;
  new_flags.internal.root_node_modules_dir_override =
    Some(dir.join("node_modules"));
  new_flags.config_flag =
    ConfigFlag::Path(dir.join("deno.json").to_string_lossy().into_owned());
  let entrypoint_flags = InstallEntrypointsFlags {
    lockfile_only: false,
    entrypoints: vec![bin_name_and_url.module_url.to_string()],
    production: false,
    skip_types: false,
  };
  new_flags.subcommand = DenoSubcommand::Install(InstallFlags::Local(
    InstallFlagsLocal::Entrypoints(entrypoint_flags.clone()),
  ));

  crate::tools::installer::install_from_entrypoints(
    Arc::new(new_flags),
    entrypoint_flags,
  )
  .await?;

  Ok(())
}

fn create_install_shim(
  bin_name_and_url: &BinaryNameAndUrl,
  cwd: &Path,
  flags: &Flags,
  install_flags_global: &InstallFlagsGlobal,
) -> Result<(), AnyError> {
  let shim_data =
    resolve_shim_data(bin_name_and_url, cwd, flags, install_flags_global)?;

  // ensure directory exists
  if let Ok(metadata) = fs::metadata(&shim_data.installation_dir) {
    if !metadata.is_dir() {
      return Err(anyhow!("Installation path is not a directory"));
    }
  } else {
    fs::create_dir_all(&shim_data.installation_dir)?;
  };

  if shim_data.file_path.exists() && !install_flags_global.force {
    return Err(anyhow!(
      "Existing installation found. Aborting (Use -f to overwrite).",
    ));
  };

  generate_executable_file(&shim_data)?;

  log::info!("✅ Successfully installed {}", bin_name_and_url.name);
  log::info!("{}", shim_data.file_path.display());
  if cfg!(windows) {
    let display_path = shim_data.file_path.with_extension("");
    log::info!("{} (shell)", display_path.display());
  }
  let installation_dir_str = shim_data.installation_dir.to_string_lossy();

  if !is_in_path(&shim_data.installation_dir) {
    log::info!("ℹ️  Add {} to PATH", installation_dir_str);
    if cfg!(windows) {
      log::info!("    set PATH=%PATH%;{}", installation_dir_str);
    } else {
      log::info!("    export PATH=\"{}:$PATH\"", installation_dir_str);
    }
  }

  Ok(())
}

fn resolve_shim_data(
  bin_name_and_url: &BinaryNameAndUrl,
  cwd: &Path,
  flags: &Flags,
  install_flags_global: &InstallFlagsGlobal,
) -> Result<ShimData, AnyError> {
  let installation_dir =
    get_installer_bin_dir(cwd, install_flags_global.root.as_deref())?;

  let mut file_path = installation_dir.join(&bin_name_and_url.name);

  if cfg!(windows) {
    file_path = file_path.with_extension("cmd");
  }

  let mut executable_args = vec!["run".to_string()];
  executable_args.extend_from_slice(&flags.to_permission_args());
  if let Some(url) = flags.location.as_ref() {
    executable_args.push("--location".to_string());
    executable_args.push(url.to_string());
  }
  if let Some(deno_lib::args::CaData::File(ca_file)) = &flags.ca_data {
    executable_args.push("--cert".to_string());
    executable_args.push(ca_file.to_owned())
  }
  if let Some(log_level) = flags.log_level {
    if log_level == Level::Error {
      executable_args.push("--quiet".to_string());
    } else {
      executable_args.push("--log-level".to_string());
      let log_level = match log_level {
        Level::Debug => "debug",
        Level::Info => "info",
        _ => return Err(anyhow!(format!("invalid log level {log_level}"))),
      };
      executable_args.push(log_level.to_string());
    }
  }

  // we should avoid a default branch here to ensure we continue to cover any
  // changes to this flag.
  match flags.type_check_mode {
    TypeCheckMode::All => executable_args.push("--check=all".to_string()),
    TypeCheckMode::None => {}
    TypeCheckMode::Local => executable_args.push("--check".to_string()),
  }

  for feature in &flags.unstable_config.features {
    executable_args.push(format!("--unstable-{}", feature));
  }

  if flags.no_remote {
    executable_args.push("--no-remote".to_string());
  }

  if flags.no_npm {
    executable_args.push("--no-npm".to_string());
  }

  if flags.cached_only {
    executable_args.push("--cached-only".to_string());
  }

  if flags.frozen_lockfile.unwrap_or(false) {
    executable_args.push("--frozen".to_string());
  }

  if resolve_no_prompt(&flags.permissions) {
    executable_args.push("--no-prompt".to_string());
  }

  if !flags.v8_flags.is_empty() {
    executable_args.push(format!("--v8-flags={}", flags.v8_flags.join(",")));
  }

  if let Some(seed) = flags.seed {
    executable_args.push("--seed".to_string());
    executable_args.push(seed.to_string());
  }

  if let Some(inspect) = flags.inspect {
    executable_args.push(format!("--inspect={inspect}"));
  }

  if let Some(inspect_brk) = flags.inspect_brk {
    executable_args.push(format!("--inspect-brk={inspect_brk}"));
  }

  if let Some(import_map_path) = &flags.import_map_path {
    let import_map_url = resolve_url_or_path(import_map_path, cwd)?;
    executable_args.push("--import-map".to_string());
    executable_args.push(import_map_url.to_string());
  }

  // all config/lock files live under .<name>/ in the bin dir
  let config_dir =
    installation_dir.join(format!(".{}", bin_name_and_url.config_dir_name()));

  let deno_json_path = config_dir.join("deno.json");
  executable_args.push("--config".to_string());
  executable_args.push(deno_json_path.to_string_lossy().into_owned());

  if let Some(node_modules_dir) = flags.node_modules_dir {
    executable_args
      .push(format!("--node-modules-dir={}", node_modules_dir.as_str()));
  }

  if flags.no_lock {
    executable_args.push("--no-lock".to_string());
  }

  executable_args.push(bin_name_and_url.module_url.to_string());
  executable_args.extend_from_slice(&install_flags_global.args);

  Ok(ShimData {
    installation_dir,
    file_path,
    args: executable_args,
  })
}

struct BinaryNameAndUrl {
  name: String,
  module_url: Url,
  /// For extra bin entries, this is the primary bin name (used for config dir).
  /// None means this is the primary entry (config dir uses self.name).
  config_name: Option<String>,
}

impl BinaryNameAndUrl {
  /// Returns the name to use for the config directory.
  fn config_dir_name(&self) -> &str {
    self.config_name.as_deref().unwrap_or(&self.name)
  }

  pub async fn resolve(
    bin_name_resolver: &BinNameResolver<'_>,
    cwd: &Path,
    module_url: &str,
    install_flags_global: &InstallFlagsGlobal,
  ) -> Result<(Self, Vec<Self>), AnyError> {
    static EXEC_NAME_RE: Lazy<Regex> = Lazy::new(|| {
      RegexBuilder::new(r"^[a-z0-9][\w-]*$")
        .case_insensitive(true)
        .build()
        .expect("invalid regex")
    });

    fn validate_name(exec_name: &str) -> Result<(), AnyError> {
      if EXEC_NAME_RE.is_match(exec_name) {
        Ok(())
      } else {
        Err(anyhow!("Invalid executable name: {exec_name}"))
      }
    }

    let module_url = resolve_url_or_path(module_url, cwd)?;
    let name = if install_flags_global.name.is_some() {
      install_flags_global.name.clone()
    } else {
      bin_name_resolver.infer_name_from_url(&module_url).await
    };
    let name = match name {
      Some(name) => name,
      None => {
        return Err(anyhow!(
          "An executable name was not provided. One could not be inferred from the URL. Aborting.\n  {} {}",
          deno_runtime::colors::cyan("hint:"),
          "provide one with the `--name` flag"
        ));
      }
    };
    validate_name(&name)?;

    // Skip extra bin resolution when --name is provided (explicit single-entry
    // intent) or when the specifier has a sub_path (e.g. npm:cowsay/cowthink).
    let mut extra_entries = Vec::new();
    if install_flags_global.name.is_none()
      && let Ok(npm_ref) = NpmPackageReqReference::from_specifier(&module_url)
      && npm_ref.sub_path().is_none()
      && let Some(all_bins) = bin_name_resolver
        .resolve_all_bin_entries_from_npm(&module_url)
        .await
      && all_bins.len() > 1
    {
      let req = npm_ref.req();
      for (bin_name, script_path) in &all_bins {
        if *bin_name == name {
          continue; // skip the primary entry
        }
        validate_name(bin_name)?;
        // Strip leading "./" from script paths (common in package.json bin fields)
        let script_path = script_path
          .strip_prefix("./")
          .unwrap_or(script_path.as_str());
        // Construct the module URL for this bin entry: npm:package@version/script_path
        let extra_url = if req.version_req.version_text() == "*" {
          Url::parse(&format!("npm:{}/{}", req.name, script_path))
        } else {
          Url::parse(&format!(
            "npm:{}@{}/{}",
            req.name,
            req.version_req.version_text(),
            script_path
          ))
        };
        if let Ok(extra_url) = extra_url {
          extra_entries.push(BinaryNameAndUrl {
            name: bin_name.clone(),
            module_url: extra_url,
            config_name: Some(name.clone()),
          });
        }
      }
    }

    extra_entries.sort_by(|a, b| a.name.cmp(&b.name));

    Ok((
      BinaryNameAndUrl {
        name,
        module_url,
        config_name: None,
      },
      extra_entries,
    ))
  }
}

struct ShimData {
  installation_dir: PathBuf,
  file_path: PathBuf,
  args: Vec<String>,
}

#[cfg(windows)]
/// On Windows, 2 files are generated.
/// One compatible with cmd & powershell with a .cmd extension
/// A second compatible with git bash / MINGW64
/// Generate batch script to satisfy that.
fn generate_executable_file(shim_data: &ShimData) -> Result<(), AnyError> {
  let args: Vec<String> =
    shim_data.args.iter().map(|c| format!("\"{c}\"")).collect();
  let template = format!(
    "% generated by deno install %\n@deno {} %*\n",
    args
      .iter()
      .map(|arg| arg.replace('%', "%%"))
      .collect::<Vec<_>>()
      .join(" ")
  );
  let mut file = File::create(&shim_data.file_path)?;
  file.write_all(template.as_bytes())?;

  // write file for bash
  // create filepath without extensions
  let template = format!(
    r#"#!/bin/sh
# generated by deno install
deno {} "$@"
"#,
    args.join(" "),
  );
  let mut file = File::create(shim_data.file_path.with_extension(""))?;
  file.write_all(template.as_bytes())?;
  Ok(())
}

#[cfg(not(windows))]
fn generate_executable_file(shim_data: &ShimData) -> Result<(), AnyError> {
  use shell_escape::escape;
  let args: Vec<String> = shim_data
    .args
    .iter()
    .map(|c| escape(c.into()).into_owned())
    .collect();
  let template = format!(
    r#"#!/bin/sh
# generated by deno install
exec deno {} "$@"
"#,
    args.join(" "),
  );
  let mut file = File::create(&shim_data.file_path)?;
  file.write_all(template.as_bytes())?;
  let _metadata = fs::metadata(&shim_data.file_path)?;
  let mut permissions = _metadata.permissions();
  permissions.set_mode(0o755);
  fs::set_permissions(&shim_data.file_path, permissions)?;
  Ok(())
}

fn get_installer_bin_dir(
  cwd: &Path,
  root_flag: Option<&str>,
) -> Result<PathBuf, AnyError> {
  let root = if let Some(root) = root_flag {
    canonicalize_path_maybe_not_exists(&cwd.join(root))?
  } else {
    get_installer_root()?
  };

  Ok(if !root.ends_with("bin") {
    root.join("bin")
  } else {
    root
  })
}

fn get_installer_root() -> Result<PathBuf, AnyError> {
  if let Some(env_dir) = env::var_os("DENO_INSTALL_ROOT")
    && !env_dir.is_empty()
  {
    let env_dir = PathBuf::from(env_dir);
    return canonicalize_path_maybe_not_exists(&env_dir).with_context(|| {
      format!(
        "Canonicalizing DENO_INSTALL_ROOT ('{}').",
        env_dir.display()
      )
    });
  }
  // Note: on Windows, the $HOME environment variable may be set by users or by
  // third party software, but it is non-standard and should not be relied upon.
  let home_env_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
  let mut home_path =
    env::var_os(home_env_var)
      .map(PathBuf::from)
      .ok_or_else(|| {
        io::Error::new(
          io::ErrorKind::NotFound,
          format!("${home_env_var} is not defined"),
        )
      })?;
  home_path.push(".deno");
  Ok(home_path)
}

/// Remove all shim files for a given name (the main file plus .cmd/.exe on Windows).
fn remove_shim_files(
  installation_dir: &Path,
  name: &str,
) -> Result<(), AnyError> {
  let path = installation_dir.join(name);
  remove_file_if_exists(&path)?;
  if cfg!(windows) {
    remove_file_if_exists(&path.with_extension("cmd"))?;
    remove_file_if_exists(&path.with_extension("exe"))?;
  }
  Ok(())
}

fn remove_file_if_exists(file_path: &Path) -> Result<bool, AnyError> {
  if let Err(err) = fs::remove_file(file_path) {
    if err.kind() == ErrorKind::NotFound {
      return Ok(false);
    }
    return Err(err)
      .with_context(|| format!("Failed removing: {}", file_path.display()));
  }
  log::info!("deleted {}", file_path.display());
  Ok(true)
}

fn get_hidden_file_with_ext(file_path: &Path, ext: &str) -> PathBuf {
  // use a dot file to prevent the file from showing up in some
  // users shell auto-complete since this directory is on the PATH
  file_path
    .with_file_name(format!(
      ".{}",
      file_path.file_name().unwrap().to_string_lossy()
    ))
    .with_extension(ext)
}

fn is_in_path(dir: &Path) -> bool {
  if let Some(paths) = env::var_os("PATH") {
    for p in env::split_paths(&paths) {
      if *dir == p {
        return true;
      }
    }
  }
  false
}

struct JsrLockfileFetcher<'a> {
  jsr_resolver: Arc<JsrFetchResolver>,
  file_fetcher: Arc<CliFileFetcher>,
  npmrc: Arc<deno_npmrc::ResolvedNpmRc>,
  npm_package_info_provider: &'a dyn deno_lockfile::NpmPackageInfoProvider,
}

impl JsrLockfileFetcher<'_> {
  async fn fetch_lockfile(&self, module_url: &Url) -> Option<String> {
    let pkg_ref = JsrPackageReqReference::from_specifier(module_url).ok()?;
    let req = pkg_ref.req();
    let nv = self.jsr_resolver.req_to_nv(req).await.ok().flatten()?;
    let lockfile_url = crate::args::jsr_url()
      .join(&format!("{}/{}/deno.lock", &nv.name, &nv.version))
      .ok()?;
    let file = match self
      .file_fetcher
      .fetch_bypass_permissions(&lockfile_url)
      .await
    {
      Ok(file) => file,
      Err(err) => {
        log::debug!("Not using lockfile for JSR package {}: {}", nv, err);
        return None;
      }
    };

    let content = match std::str::from_utf8(&file.source) {
      Ok(s) => s,
      Err(_) => {
        log::debug!("Lockfile for JSR package {} is not valid UTF-8", nv);
        return None;
      }
    };

    // Parse and upgrade the lockfile to v5
    let lockfile = match deno_lockfile::Lockfile::new(
      deno_lockfile::NewLockfileOptions {
        file_path: std::path::PathBuf::from("deno.lock"),
        content,
        overwrite: false,
      },
      self.npm_package_info_provider,
    )
    .await
    {
      Ok(lockfile) => lockfile,
      Err(err) => {
        log::warn!(
          "{} Not using lockfile from JSR package {}: {}",
          crate::colors::yellow("Warning"),
          nv,
          err,
        );
        return None;
      }
    };

    if let Err(url) = validate_npm_tarball_urls(&lockfile.content, &self.npmrc)
    {
      log::warn!(
        "{} Not using lockfile from JSR package {} because it contains an npm tarball URL (\"{}\") not from a configured npm registry. This may indicate a security issue.",
        crate::colors::yellow("Warning"),
        nv,
        url,
      );
      return None;
    }

    log::debug!("Using lockfile from JSR package {}", nv);
    Some(lockfile.as_json_string())
  }
}

/// Validates that all npm tarball URLs in the lockfile come from
/// configured npm registries (or the default registry.npmjs.org).
/// Returns `Err(bad_url)` on the first tarball from an unknown registry.
fn validate_npm_tarball_urls(
  content: &deno_lockfile::LockfileContent,
  npmrc: &deno_npmrc::ResolvedNpmRc,
) -> Result<(), String> {
  let mut allowed_registries = npmrc.get_all_known_registries_urls();
  // always allow the default npm registry
  let default_npm_registry =
    Url::parse(deno_npmrc::NPM_DEFAULT_REGISTRY).unwrap();
  if !allowed_registries.contains(&default_npm_registry) {
    allowed_registries.push(default_npm_registry);
  }

  for pkg_info in content.packages.npm.values() {
    if let Some(tarball) = &pkg_info.tarball {
      let tarball_str = tarball.as_str();
      let is_allowed = allowed_registries
        .iter()
        .any(|registry_url| tarball_str.starts_with(registry_url.as_str()));
      if !is_allowed {
        return Err(tarball_str.to_string());
      }
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::process::Command;

  use deno_lib::args::UnstableConfig;
  use deno_npm::resolution::NpmVersionResolver;
  use test_util::TempDir;
  use test_util::testdata_path;

  use super::*;
  use crate::args::ConfigFlag;
  use crate::args::PermissionFlags;
  use crate::args::UninstallFlagsGlobal;
  use crate::http_util::HttpClientProvider;
  use crate::util::env::resolve_cwd;
  use crate::util::fs::canonicalize_path;

  async fn create_install_shim(
    flags: &Flags,
    install_flags_global: InstallFlagsGlobal,
  ) -> Result<(), AnyError> {
    let _http_server_guard = test_util::http_server();
    let cwd = resolve_cwd(None).unwrap();
    let http_client = HttpClientProvider::new(None, None);
    let registry_api = deno_npm::registry::TestNpmRegistryApi::default();
    let npm_version_resolver = NpmVersionResolver::default();
    let resolver =
      BinNameResolver::new(&http_client, &registry_api, &npm_version_resolver);
    let (binary_name_and_url, _extra_entries) = BinaryNameAndUrl::resolve(
      &resolver,
      &cwd,
      &install_flags_global.module_urls[0],
      &install_flags_global,
    )
    .await?;
    let installation_dir =
      super::get_installer_bin_dir(&cwd, install_flags_global.root.as_deref())
        .unwrap();
    super::setup_config_dir(
      &binary_name_and_url,
      flags,
      &installation_dir,
      None,
    )
    .await
    .unwrap();
    super::create_install_shim(
      &binary_name_and_url,
      &cwd,
      flags,
      &install_flags_global,
    )
  }

  /// Returns the config directory path (e.g. `<root>/bin/.<name>/`) for a given
  /// root and binary name.
  fn config_dir_for(root: &str, name: &str) -> PathBuf {
    let cwd = resolve_cwd(None).unwrap();
    super::get_installer_bin_dir(&cwd, Some(root))
      .unwrap()
      .join(format!(".{name}"))
  }

  async fn resolve_shim_data(
    flags: &Flags,
    install_flags_global: &InstallFlagsGlobal,
  ) -> Result<(BinaryNameAndUrl, ShimData), AnyError> {
    let _http_server_guard = test_util::http_server();
    let cwd = resolve_cwd(None).unwrap();
    let http_client = HttpClientProvider::new(None, None);
    let registry_api = deno_npm::registry::TestNpmRegistryApi::default();
    let npm_version_resolver = NpmVersionResolver::default();
    let resolver =
      BinNameResolver::new(&http_client, &registry_api, &npm_version_resolver);
    let (binary_name_and_url, _extra_entries) = BinaryNameAndUrl::resolve(
      &resolver,
      &cwd,
      &install_flags_global.module_urls[0],
      install_flags_global,
    )
    .await?;
    let shim_data = super::resolve_shim_data(
      &binary_name_and_url,
      &cwd,
      flags,
      install_flags_global,
    )?;
    Ok((binary_name_and_url, shim_data))
  }

  #[tokio::test]
  async fn install_unstable() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();

    create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }

    assert!(file_path.exists());

    let content = fs::read_to_string(file_path).unwrap();
    let config_path = config_dir_for(&temp_dir.path().to_string(), "echo_test")
      .join("deno.json");
    if cfg!(windows) {
      assert!(content.contains(&format!(
        r#""run" "--config" "{}" "http://localhost:4545/echo.ts""#,
        config_path.to_string_lossy()
      )));
    } else {
      assert!(content.contains(&format!(
        "run --config {} 'http://localhost:4545/echo.ts'",
        config_path.to_string_lossy()
      )));
    }
  }

  #[tokio::test]
  async fn install_inferred_name() {
    let temp_dir_str = env::temp_dir().to_string_lossy().into_owned();
    let config_path = config_dir_for(&temp_dir_str, "echo").join("deno.json");
    let (bin_info, shim_data) = resolve_shim_data(
      &Flags::default(),
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo.ts".to_string()],
        args: vec![],
        name: None,
        root: Some(temp_dir_str),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(bin_info.name, "echo");
    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--config",
        &config_path.to_string_lossy(),
        "http://localhost:4545/echo.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_unstable_legacy() {
    let temp_dir_str = env::temp_dir().to_string_lossy().into_owned();
    let config_path = config_dir_for(&temp_dir_str, "echo").join("deno.json");
    let (bin_info, shim_data) = resolve_shim_data(
      &Default::default(),
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo.ts".to_string()],
        args: vec![],
        name: None,
        root: Some(temp_dir_str),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(bin_info.name, "echo");
    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--config",
        &config_path.to_string_lossy(),
        "http://localhost:4545/echo.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_unstable_features() {
    let temp_dir_str = env::temp_dir().to_string_lossy().into_owned();
    let config_path = config_dir_for(&temp_dir_str, "echo").join("deno.json");
    let (bin_info, shim_data) = resolve_shim_data(
      &Flags {
        unstable_config: UnstableConfig {
          features: vec!["kv".to_string(), "cron".to_string()],
          ..Default::default()
        },
        ..Default::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo.ts".to_string()],
        args: vec![],
        name: None,
        root: Some(temp_dir_str),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(bin_info.name, "echo");
    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--unstable-kv",
        "--unstable-cron",
        "--config",
        &config_path.to_string_lossy(),
        "http://localhost:4545/echo.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_inferred_name_from_parent() {
    let temp_dir_str = env::temp_dir().to_string_lossy().into_owned();
    let config_path = config_dir_for(&temp_dir_str, "subdir").join("deno.json");
    let (bin_info, shim_data) = resolve_shim_data(
      &Flags::default(),
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/subdir/main.ts".to_string()],
        args: vec![],
        name: None,
        root: Some(temp_dir_str),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(bin_info.name, "subdir");
    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--config",
        &config_path.to_string_lossy(),
        "http://localhost:4545/subdir/main.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_inferred_name_after_redirect_for_no_path_url() {
    let _http_server_guard = test_util::http_server();
    let temp_dir_str = env::temp_dir().to_string_lossy().into_owned();
    let config_path = config_dir_for(&temp_dir_str, "a").join("deno.json");
    let (bin_info, shim_data) = resolve_shim_data(
      &Flags::default(),
      &InstallFlagsGlobal {
        module_urls: vec![
          "http://localhost:4550/?redirect_to=/subdir/redirects/a.ts"
            .to_string(),
        ],
        args: vec![],
        name: None,
        root: Some(temp_dir_str),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(bin_info.name, "a");
    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--config",
        &config_path.to_string_lossy(),
        "http://localhost:4550/?redirect_to=/subdir/redirects/a.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_custom_dir_option() {
    let temp_dir_str = env::temp_dir().to_string_lossy().into_owned();
    let config_path =
      config_dir_for(&temp_dir_str, "echo_test").join("deno.json");
    let (bin_info, shim_data) = resolve_shim_data(
      &Flags::default(),
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir_str),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(bin_info.name, "echo_test");
    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--config",
        &config_path.to_string_lossy(),
        "http://localhost:4545/echo.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_with_flags() {
    let temp_dir_str = env::temp_dir().to_string_lossy().into_owned();
    let config_path =
      config_dir_for(&temp_dir_str, "echo_test").join("deno.json");
    let (bin_info, shim_data) = resolve_shim_data(
      &Flags {
        permissions: PermissionFlags {
          allow_net: Some(vec![]),
          allow_read: Some(vec![]),
          ..Default::default()
        },
        type_check_mode: TypeCheckMode::None,
        log_level: Some(Level::Error),
        ..Flags::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo.ts".to_string()],
        args: vec!["--foobar".to_string()],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir_str),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(bin_info.name, "echo_test");
    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--allow-read",
        "--allow-net",
        "--quiet",
        "--config",
        &config_path.to_string_lossy(),
        "http://localhost:4545/echo.ts",
        "--foobar",
      ]
    );
  }

  #[tokio::test]
  async fn install_prompt() {
    let temp_dir_str = env::temp_dir().to_string_lossy().into_owned();
    let config_path =
      config_dir_for(&temp_dir_str, "echo_test").join("deno.json");
    let (_, shim_data) = resolve_shim_data(
      &Flags {
        permissions: PermissionFlags {
          no_prompt: true,
          ..Default::default()
        },
        ..Flags::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir_str),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--no-prompt",
        "--config",
        &config_path.to_string_lossy(),
        "http://localhost:4545/echo.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_allow_all() {
    let temp_dir_str = env::temp_dir().to_string_lossy().into_owned();
    let config_path =
      config_dir_for(&temp_dir_str, "echo_test").join("deno.json");
    let (_, shim_data) = resolve_shim_data(
      &Flags {
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir_str),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--allow-all",
        "--config",
        &config_path.to_string_lossy(),
        "http://localhost:4545/echo.ts",
      ]
    );
  }

  #[tokio::test]
  async fn install_npm_lockfile_default() {
    let temp_dir_str = env::temp_dir().to_string_lossy().into_owned();
    let config_path = config_dir_for(&temp_dir_str, "cowsay").join("deno.json");
    let (_, shim_data) = resolve_shim_data(
      &Flags {
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        ..Flags::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["npm:cowsay".to_string()],
        args: vec![],
        name: None,
        root: Some(temp_dir_str),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--allow-all",
        "--config",
        &config_path.to_string_lossy(),
        "npm:cowsay"
      ]
    );
  }

  #[tokio::test]
  async fn install_npm_no_lock() {
    let temp_dir_str = env::temp_dir().to_string_lossy().into_owned();
    let config_path = config_dir_for(&temp_dir_str, "cowsay").join("deno.json");
    let (_, shim_data) = resolve_shim_data(
      &Flags {
        permissions: PermissionFlags {
          allow_all: true,
          ..Default::default()
        },
        no_lock: true,
        ..Flags::default()
      },
      &InstallFlagsGlobal {
        module_urls: vec!["npm:cowsay".to_string()],
        args: vec![],
        name: None,
        root: Some(temp_dir_str),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    assert_eq!(
      shim_data.args,
      vec![
        "run",
        "--allow-all",
        "--config",
        &config_path.to_string_lossy(),
        "--no-lock",
        "npm:cowsay"
      ]
    );
  }

  #[tokio::test]
  async fn install_local_module() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();
    let local_module = testdata_path().join("echo.ts");
    let local_module_url = Url::from_file_path(&local_module).unwrap();
    let local_module_str = local_module.to_string_lossy();

    create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec![local_module_str.to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }

    assert!(file_path.exists());
    let content = fs::read_to_string(file_path).unwrap();
    assert!(content.contains(&local_module_url.to_string()));
  }

  #[tokio::test]
  async fn install_force() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();

    create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }
    assert!(file_path.exists());

    // No force. Install failed.
    let no_force_result = create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/cat.ts".to_string()], // using a different URL
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
        compile: false,
      },
    )
    .await;
    assert!(no_force_result.is_err());
    assert!(
      no_force_result
        .unwrap_err()
        .to_string()
        .contains("Existing installation found")
    );
    // Assert not modified
    let file_content = fs::read_to_string(&file_path).unwrap();
    assert!(file_content.contains("echo.ts"));

    // Force. Install success.
    let force_result = create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/cat.ts".to_string()], // using a different URL
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: true,
        compile: false,
      },
    )
    .await;
    assert!(force_result.is_ok());
    // Assert modified
    let file_content_2 = fs::read_to_string(&file_path).unwrap();
    assert!(file_content_2.contains("cat.ts"));
  }

  #[tokio::test]
  async fn install_with_config() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    let config_file_path = temp_dir.path().join("test_tsconfig.json");
    let config = "{}";
    let mut config_file = File::create(&config_file_path).unwrap();
    let result = config_file.write_all(config.as_bytes());
    assert!(result.is_ok());

    let result = create_install_shim(
      &Flags {
        config_flag: ConfigFlag::Path(config_file_path.to_string()),
        ..Flags::default()
      },
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/cat.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: true,
        compile: false,
      },
    )
    .await;
    assert!(result.is_ok());

    let file_path = bin_dir.join(".echo_test").join("deno.json");
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path).unwrap();
    // setup_config_dir appends a workspace field to stop workspace discovery
    assert!(content.contains("\"workspace\""));
  }

  // TODO: enable on Windows after fixing batch escaping
  #[cfg(not(windows))]
  #[tokio::test]
  async fn install_shell_escaping() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();

    create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo.ts".to_string()],
        args: vec!["\"".to_string()],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }

    assert!(file_path.exists());
    let content = fs::read_to_string(file_path).unwrap();
    let config_path = config_dir_for(&temp_dir.path().to_string(), "echo_test")
      .join("deno.json");
    if cfg!(windows) {
      // TODO: see comment above this test
    } else {
      assert!(content.contains(&format!(
        "run --config {} 'http://localhost:4545/echo.ts' '\"'",
        config_path.to_string_lossy()
      )));
    }
  }

  #[tokio::test]
  async fn install_unicode() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();
    let unicode_dir = temp_dir.path().join("Magnús");
    std::fs::create_dir(&unicode_dir).unwrap();
    let local_module = unicode_dir.join("echo.ts");
    let local_module_str = local_module.to_string_lossy();
    std::fs::write(&local_module, "// Some JavaScript I guess").unwrap();

    create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec![local_module_str.to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: false,
        compile: false,
      },
    )
    .await
    .unwrap();

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }

    // We need to actually run it to make sure the URL is interpreted correctly
    let status = Command::new(file_path)
      .env_clear()
      // use the deno binary in the target directory
      .env("PATH", test_util::target_dir())
      .env("RUST_BACKTRACE", "1")
      .spawn()
      .unwrap()
      .wait()
      .unwrap();
    assert!(status.success());
  }

  #[tokio::test]
  async fn install_with_import_map() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    let import_map_path = temp_dir.path().join("import_map.json");
    let import_map_url = Url::from_file_path(&import_map_path).unwrap();
    let import_map = "{ \"imports\": {} }";
    let mut import_map_file = File::create(&import_map_path).unwrap();
    let result = import_map_file.write_all(import_map.as_bytes());
    assert!(result.is_ok());

    let result = create_install_shim(
      &Flags {
        import_map_path: Some(import_map_path.to_string()),
        ..Flags::default()
      },
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/cat.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: true,
        compile: false,
      },
    )
    .await;
    assert!(result.is_ok());

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }
    assert!(file_path.exists());

    let config_path = config_dir_for(&temp_dir.path().to_string(), "echo_test")
      .join("deno.json");
    let mut expected_string = format!(
      "--import-map '{import_map_url}' --config {} 'http://localhost:4545/cat.ts'",
      config_path.to_string_lossy()
    );
    if cfg!(windows) {
      expected_string = format!(
        "\"--import-map\" \"{import_map_url}\" \"--config\" \"{}\" \"http://localhost:4545/cat.ts\"",
        config_path.to_string_lossy()
      );
    }

    let content = fs::read_to_string(file_path).unwrap();
    assert!(content.contains(&expected_string));
  }

  // Regression test for https://github.com/denoland/deno/issues/10556.
  #[tokio::test]
  async fn install_file_url() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    let module_path =
      canonicalize_path(testdata_path().join("cat.ts").as_path()).unwrap();
    let file_module_string =
      Url::from_file_path(module_path).unwrap().to_string();
    assert!(file_module_string.starts_with("file:///"));

    let result = create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec![file_module_string.to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: true,
        compile: false,
      },
    )
    .await;
    assert!(result.is_ok());

    let mut file_path = bin_dir.join("echo_test");
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
    }
    assert!(file_path.exists());

    let config_path = config_dir_for(&temp_dir.path().to_string(), "echo_test")
      .join("deno.json");
    let mut expected_string = format!(
      "run --config {} '{}'",
      config_path.to_string_lossy(),
      &file_module_string
    );
    if cfg!(windows) {
      expected_string = format!(
        "\"run\" \"--config\" \"{}\" \"{}\"",
        config_path.to_string_lossy(),
        &file_module_string
      );
    }

    let content = fs::read_to_string(file_path).unwrap();
    assert!(content.contains(&expected_string));
  }

  #[tokio::test]
  async fn uninstall_basic() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();

    let mut file_path = bin_dir.join("echo_test");
    File::create(&file_path).unwrap();
    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
      File::create(&file_path).unwrap();
    }
    let shim_path = file_path.clone();

    // create extra files
    {
      let file_path = file_path.with_extension("deno.json");
      File::create(file_path).unwrap();
    }
    {
      // legacy tsconfig.json, make sure it's cleaned up for now
      let file_path = file_path.with_extension("tsconfig.json");
      File::create(file_path).unwrap();
    }
    {
      let file_path = file_path.with_extension("lock.json");
      File::create(file_path).unwrap();
    }

    // create hidden per-command copies as produced by install
    {
      let hidden_file =
        get_hidden_file_with_ext(shim_path.as_path(), "deno.json");
      File::create(hidden_file).unwrap();
    }
    {
      let hidden_file =
        get_hidden_file_with_ext(shim_path.as_path(), "lock.json");
      File::create(hidden_file).unwrap();
    }

    uninstall(
      Default::default(),
      UninstallFlags {
        kind: UninstallKind::Global(UninstallFlagsGlobal {
          name: "echo_test".to_string(),
          root: Some(temp_dir.path().to_string()),
        }),
      },
    )
    .await
    .unwrap();

    assert!(!file_path.exists());
    assert!(!file_path.with_extension("tsconfig.json").exists());
    assert!(!file_path.with_extension("deno.json").exists());
    assert!(!file_path.with_extension("lock.json").exists());

    // hidden per-command files should also be removed
    assert!(
      !get_hidden_file_with_ext(shim_path.as_path(), "deno.json").exists()
    );
    assert!(
      !get_hidden_file_with_ext(shim_path.as_path(), "lock.json").exists()
    );

    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
      assert!(!file_path.exists());
    }
  }

  fn create_npmrc_with_registries(
    default_url: &str,
    scope_urls: &[(&str, &str)],
  ) -> Arc<deno_npmrc::ResolvedNpmRc> {
    use deno_npmrc::RegistryConfig;
    use deno_npmrc::RegistryConfigWithUrl;
    use deno_npmrc::ResolvedNpmRc;

    let mut scopes = std::collections::HashMap::new();
    for (scope, url) in scope_urls {
      scopes.insert(
        scope.to_string(),
        RegistryConfigWithUrl {
          registry_url: Url::parse(url).unwrap(),
          config: Arc::new(RegistryConfig::default()),
        },
      );
    }
    Arc::new(ResolvedNpmRc {
      default_config: RegistryConfigWithUrl {
        registry_url: Url::parse(default_url).unwrap(),
        config: Arc::new(RegistryConfig::default()),
      },
      scopes,
      registry_configs: Default::default(),
    })
  }

  fn create_lockfile_content_with_npm(
    packages: &[(&str, Option<&str>)],
  ) -> deno_lockfile::LockfileContent {
    use deno_lockfile::NpmPackageInfo;
    let mut content = deno_lockfile::LockfileContent::default();
    for (id, tarball) in packages {
      content.packages.npm.insert(
        (*id).into(),
        NpmPackageInfo {
          integrity: Some("sha512-test".to_string()),
          dependencies: Default::default(),
          optional_dependencies: Default::default(),
          optional_peers: Default::default(),
          os: Default::default(),
          cpu: Default::default(),
          tarball: tarball.map(|t| t.into()),
          deprecated: false,
          scripts: false,
          bin: false,
        },
      );
    }
    content
  }

  #[test]
  fn validate_npm_tarball_urls_allows_default_registry() {
    let npmrc =
      create_npmrc_with_registries("https://registry.npmjs.org/", &[]);
    let content = create_lockfile_content_with_npm(&[(
      "chalk@5.0.0",
      Some("https://registry.npmjs.org/chalk/-/chalk-5.0.0.tgz"),
    )]);
    assert!(super::validate_npm_tarball_urls(&content, &npmrc).is_ok());
  }

  #[test]
  fn validate_npm_tarball_urls_allows_no_tarball() {
    let npmrc =
      create_npmrc_with_registries("https://registry.npmjs.org/", &[]);
    let content = create_lockfile_content_with_npm(&[("chalk@5.0.0", None)]);
    assert!(super::validate_npm_tarball_urls(&content, &npmrc).is_ok());
  }

  #[test]
  fn validate_npm_tarball_urls_rejects_unknown_registry() {
    let npmrc =
      create_npmrc_with_registries("https://registry.npmjs.org/", &[]);
    let content = create_lockfile_content_with_npm(&[(
      "evil@1.0.0",
      Some("https://evil.example.com/evil/-/evil-1.0.0.tgz"),
    )]);
    let result = super::validate_npm_tarball_urls(&content, &npmrc);
    assert_eq!(
      result.unwrap_err(),
      "https://evil.example.com/evil/-/evil-1.0.0.tgz"
    );
  }

  #[test]
  fn validate_npm_tarball_urls_allows_scoped_registry() {
    let npmrc = create_npmrc_with_registries(
      "https://registry.npmjs.org/",
      &[("myco", "https://npm.mycompany.com/")],
    );
    let content = create_lockfile_content_with_npm(&[(
      "@myco/pkg@1.0.0",
      Some("https://npm.mycompany.com/@myco/pkg/-/pkg-1.0.0.tgz"),
    )]);
    assert!(super::validate_npm_tarball_urls(&content, &npmrc).is_ok());
  }

  #[test]
  fn validate_npm_tarball_urls_allows_npmjs_when_custom_default() {
    // Even with a custom default registry, registry.npmjs.org should be allowed
    let npmrc = create_npmrc_with_registries("https://npm.mycompany.com/", &[]);
    let content = create_lockfile_content_with_npm(&[(
      "chalk@5.0.0",
      Some("https://registry.npmjs.org/chalk/-/chalk-5.0.0.tgz"),
    )]);
    assert!(super::validate_npm_tarball_urls(&content, &npmrc).is_ok());
  }

  #[test]
  fn validate_npm_tarball_urls_mixed_valid_and_invalid() {
    let npmrc =
      create_npmrc_with_registries("https://registry.npmjs.org/", &[]);
    let content = create_lockfile_content_with_npm(&[
      (
        "chalk@5.0.0",
        Some("https://registry.npmjs.org/chalk/-/chalk-5.0.0.tgz"),
      ),
      (
        "evil@1.0.0",
        Some("https://evil.example.com/evil/-/evil-1.0.0.tgz"),
      ),
    ]);
    assert!(super::validate_npm_tarball_urls(&content, &npmrc).is_err());
  }

  #[test]
  fn validate_npm_tarball_urls_rejects_subdomain_spoof() {
    // ensure "https://registry.npmjs.org" (no trailing slash) doesn't
    // match "https://registry.npmjs.org.evil.com/..." via prefix
    let npmrc = create_npmrc_with_registries("https://registry.npmjs.org", &[]);
    let content = create_lockfile_content_with_npm(&[(
      "evil@1.0.0",
      Some("https://registry.npmjs.org.evil.com/evil/-/evil-1.0.0.tgz"),
    )]);
    assert!(super::validate_npm_tarball_urls(&content, &npmrc).is_err());
  }
}
