// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
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
use crate::args::FlagsExt;
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
      Arc::new(deno_runtime::deno_web::BlobStore::default())
        as Arc<dyn deno_runtime::deno_web::BlobStoreTrait>,
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

  // When an explicit config is supplied, capture its workspace members so they
  // can be flattened into the copied config's import map. The `workspace` field
  // itself is dropped from the copy (it can't be discovered from the install
  // dir), which would otherwise break resolution of member packages.
  let workspace_member_imports =
    if matches!(flags.config_flag, ConfigFlag::Path(_)) {
      workspace_member_import_entries(cli_options.workspace())
    } else {
      Vec::new()
    };

  // Validate every entry and default unprefixed bare package names to the npm
  // registry (matching `deno add` and local `deno install`) before installing
  // anything, so an error on entry N doesn't leave entries < N installed.
  let module_urls: Vec<String> = install_flags_global
    .module_urls
    .iter()
    .enumerate()
    .map(|(i, module_url)| -> Result<String, AnyError> {
      let entry_text = module_url;
      if cli_options.initial_cwd().join(entry_text).exists() {
        return Ok(module_url.clone());
      }
      // Migration error for users coming from Deno < 3.0 who passed script
      // args without `--`.
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
        );
      }
      if let Ok(Err(package_req)) =
        crate::tools::pm::AddRmPackageReq::parse(entry_text, None)
      {
        return Ok(format!("npm:{package_req}"));
      }
      Ok(module_url.clone())
    })
    .collect::<Result<_, _>>()?;

  for module_url in &module_urls {
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
    // Flatten dependencies from the entrypoint's closest package.json into the
    // copied config's import map. Entries from an explicit `--config` import
    // map and from workspace members take precedence.
    let mut extra_imports = workspace_member_imports.clone();
    extra_imports
      .extend(package_json_dep_import_entries(&name_and_url.module_url));
    setup_config_dir(
      &name_and_url,
      &flags,
      cli_options.initial_cwd(),
      &installation_dir,
      Some(&jsr_lockfile_fetcher),
      install_flags_global.force,
      &extra_imports,
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

  for name in &uninstall_flags.packages {
    let file_path = installation_dir.join(name);

    let mut removed = remove_file_if_exists(&file_path)?;

    if cfg!(windows) {
      removed |= remove_file_if_exists(&file_path.with_extension("cmd"))?;
      removed |= remove_file_if_exists(&file_path.with_extension("exe"))?;
    }

    if !removed {
      return Err(anyhow!("No installation found for {}", name));
    }

    // There might be some extra files to delete
    // Note: tsconfig.json is legacy. We renamed it to deno.json in January 2023.
    // Note: deno.json and lock.json files were removed Feb 2026 in favor of a sub directory
    // Use the base file path (without extension) to compute related files
    let base_file = installation_dir.join(name);
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
    let config_dir = installation_dir.join(format!(".{}", name));

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

    log::info!("✅ Successfully uninstalled {}", name);
  }

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
    bundle: false,
    app_name: None,
    minify: false,
    exclude_unused_npm: false,
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
  cwd: &Path,
  installation_dir: &Path,
  jsr_lockfile_fetcher: Option<&JsrLockfileFetcher<'_>>,
  force: bool,
  extra_imports: &[(String, String)],
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

  // When --force is specified, the user is explicitly asking for a fresh
  // install. Remove the stale auto-generated lockfile so dependency resolution
  // isn't constrained to previously pinned versions.
  if force {
    let lockfile_path = dir.join("deno.lock");
    if lockfile_path.exists() {
      fs::remove_file(&lockfile_path).with_context(|| {
        format!("failed removing '{}'", lockfile_path.display())
      })?;
    }
  }

  let (config_text, original_config_url) =
    if let ConfigFlag::Path(config_path) = &flags.config_flag {
      let text = fs::read_to_string(config_path)
        .with_context(|| format!("error reading {config_path}"))?;
      let cwd = resolve_cwd(flags.initial_cwd.as_deref())?;
      let url = resolve_url_or_path(config_path, &cwd)?;
      (text, Some(url))
    } else {
      ("{}\n".to_string(), None)
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
  // The copied config lives at `<installation_dir>/.<name>/deno.json`, so any
  // relative `./` / `../` paths in `imports` / `scopes` would resolve against
  // that new location instead of the original config dir, breaking module
  // resolution at runtime. Rewrite them to absolute `file://` URLs anchored to
  // the original config so the installed binary keeps importing the same
  // modules it would have when invoked directly.
  if let Some(original_config_url) = &original_config_url {
    rewrite_relative_import_map_paths(&config_obj, original_config_url);
  }
  // Flatten workspace members and package.json dependencies into the import
  // map. The `workspace` field is stripped below to stop discovery and the
  // entrypoint's package.json isn't visible from the config dir, which would
  // otherwise break resolution of bare specifiers that point at member
  // packages or package.json dependencies. Existing import map entries win,
  // matching the runtime precedence of the import map.
  if !extra_imports.is_empty() {
    let imports = config_obj.object_value_or_set("imports");
    for (specifier, target) in extra_imports {
      if imports.get(specifier).is_none() {
        imports.append(specifier, CstInputValue::String(target.clone()));
      }
    }
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

  // fetch deno.lock from JSR if this is a JSR package
  let fetched_lockfile = if !flags.no_lock
    && let Some(fetcher) = jsr_lockfile_fetcher
  {
    fetcher.fetch_lockfile(&bin_name_and_url.module_url).await
  } else {
    None
  };
  // Carry the published lockfile's workspace dependency roots into the
  // generated config's import map. Without them the local install below would
  // see an empty workspace, prune those roots from the lockfile, and re-resolve
  // the pins to newer versions (denoland/deno#33323).
  if let Some(fetched) = &fetched_lockfile {
    let imports = config_obj.object_value_or_set("imports");
    for (key, value) in &fetched.workspace_imports {
      if imports.get(key).is_none() {
        imports.append(key, CstInputValue::String(value.clone()));
      }
    }
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

  if let Some(fetched) = fetched_lockfile {
    fs::write(dir.join("deno.lock"), fetched.content)?;
  }

  // create cloned flags to run cache_top_level_deps
  let mut new_flags = flags.clone();
  // Pre-resolve cwd-relative paths against the user's original cwd before
  // switching `initial_cwd` to the generated install dir, otherwise they'd
  // be re-resolved against `dir` and point at non-existent files.
  if let Some(import_map_path) = &flags.import_map_path {
    new_flags.import_map_path =
      Some(resolve_url_or_path(import_map_path, cwd)?.to_string());
  }
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
    Default::default(),
  ));

  crate::tools::installer::install_from_entrypoints(
    Arc::new(new_flags),
    entrypoint_flags,
  )
  .await?;

  Ok(())
}

/// Builds import map entries for each workspace member package so the copied
/// config keeps resolving `@scope/member` specifiers after the `workspace`
/// field is stripped.
///
/// `deno install -g -c deno.json` copies the workspace root config into a
/// per-binary directory and removes the `workspace` field to stop workspace
/// discovery. Without this, bare specifiers that point at workspace members
/// (e.g. `import { x } from "@scope/member"`) can no longer be resolved. Each
/// member's exports are flattened into absolute `file://` specifiers anchored
/// at the member directory so the installed binary resolves them exactly as it
/// would when invoked from the original workspace. See
/// https://github.com/denoland/deno/issues/32057.
///
/// Only `deno.json` members (those with a `name` and `exports`) are flattened,
/// since they are the ones resolved through the import map. `package.json`
/// members resolve via `node_modules`, which is unaffected by stripping the
/// `workspace` field, so they don't need an entry here.
fn workspace_member_import_entries(
  workspace: &deno_config::workspace::Workspace,
) -> Vec<(String, String)> {
  let mut entries = Vec::new();
  for pkg in workspace.resolver_jsr_pkgs() {
    for (export_key, sub_path) in &pkg.exports {
      // "." -> "@scope/name", "./sub" -> "@scope/name/sub"
      let specifier = format!(
        "{}{}",
        pkg.name,
        export_key.strip_prefix('.').unwrap_or(export_key)
      );
      if let Ok(target) = pkg.base.join(sub_path) {
        entries.push((specifier, target.to_string()));
      }
    }
  }
  entries
}

/// Builds import map entries for the dependencies declared in the closest
/// package.json to a local file entrypoint.
///
/// `deno install -g` copies the supplied config (or an empty one) into a
/// per-binary directory and the installed command runs with that config, so
/// dependencies declared in the project's package.json are not visible to it.
/// Flatten them into the copied config's import map (e.g. `"@std/log":
/// "npm:@jsr/std__log@^0.224.9"`, plus a trailing-slash entry for subpath
/// imports) so bare specifiers keep resolving the same way they do when
/// running the entrypoint directly. See
/// https://github.com/denoland/deno/issues/26412.
fn package_json_dep_import_entries(module_url: &Url) -> Vec<(String, String)> {
  use deno_package_json::PackageJsonDepValue;

  let Ok(file_path) = deno_path_util::url_to_file_path(module_url) else {
    return Vec::new();
  };
  let sys = crate::sys::CliSys::default();
  let mut maybe_dir = file_path.parent();
  while let Some(dir) = maybe_dir {
    let pkg_json = match deno_package_json::PackageJson::load_from_path(
      &sys,
      None,
      &dir.join("package.json"),
    ) {
      Ok(Some(pkg_json)) => pkg_json,
      Ok(None) => {
        maybe_dir = dir.parent();
        continue;
      }
      Err(err) => {
        log::warn!(
          "{} Ignoring package.json for the installed command: {:#}",
          crate::colors::yellow("Warning"),
          err
        );
        return Vec::new();
      }
    };
    let deps = pkg_json.resolve_local_package_json_deps();
    let mut entries = Vec::new();
    // Only runtime `dependencies` are flattened. `devDependencies` aren't
    // needed by the installed global command and would otherwise emit a
    // spurious warning for any dev-only file:/workspace:/catalog: entry.
    for (alias, dep) in deps.dependencies.iter() {
      match dep {
        Ok(PackageJsonDepValue::Req(req)) => {
          // The trailing-slash entry uses the `npm:/pkg@req/` form because
          // `npm:pkg@req/` is an opaque-path URL that subpaths cannot be
          // URL-joined onto.
          entries.push((
            format!("{}/", alias),
            format!("npm:/{}@{}/", req.name, req.version_req.version_text()),
          ));
          entries.push((
            alias.to_string(),
            format!("npm:{}@{}", req.name, req.version_req.version_text()),
          ));
        }
        Ok(
          PackageJsonDepValue::File(_)
          | PackageJsonDepValue::Workspace { .. }
          | PackageJsonDepValue::Catalog(_),
        )
        | Err(_) => {
          log::warn!(
            "{} Ignoring \"{}\" from '{}' because only npm and jsr dependencies are supported by the installed command.",
            crate::colors::yellow("Warning"),
            alias,
            pkg_json.path.display(),
          );
        }
      }
    }
    return entries;
  }
  Vec::new()
}

/// Rewrites `./` and `../` paths inside `imports` and `scopes` to absolute
/// `file://` URLs anchored to the original config file's location.
///
/// `deno install -g -c deno.json` copies the supplied config into a per-binary
/// directory. Relative specifiers / scope prefixes get resolved relative to
/// that new directory by `deno_config`, so without rewriting them they end up
/// pointing at non-existent paths under the install dir. See
/// https://github.com/denoland/deno/issues/20390.
fn rewrite_relative_import_map_paths(
  config_obj: &jsonc_parser::cst::CstObject,
  original_config_url: &Url,
) {
  fn rewrite_to_absolute(value: &str, base: &Url) -> Option<String> {
    if value.starts_with("./") || value.starts_with("../") {
      base.join(value).ok().map(|u| u.to_string())
    } else {
      None
    }
  }

  fn rewrite_string_key(name: &jsonc_parser::cst::ObjectPropName, base: &Url) {
    if let jsonc_parser::cst::ObjectPropName::String(key_lit) = name
      && let Ok(current_key) = key_lit.decoded_value()
      && let Some(new_key) = rewrite_to_absolute(&current_key, base)
    {
      key_lit.set_raw_value(format!(
        "\"{}\"",
        new_key.replace('\\', "\\\\").replace('"', "\\\"")
      ));
    }
  }

  fn rewrite_specifier_map(map_obj: &jsonc_parser::cst::CstObject, base: &Url) {
    for prop in map_obj.properties() {
      // Rewrite the key if it's a `./` / `../` path. `normalize_specifier_key`
      // joins URL-like keys with the base URL, so keys would also drift to the
      // install dir without rewriting.
      if let Some(name) = prop.name() {
        rewrite_string_key(&name, base);
      }
      let Some(value_node) = prop.value() else {
        continue;
      };
      let Some(string_lit) = value_node.as_string_lit() else {
        continue;
      };
      let Ok(current) = string_lit.decoded_value() else {
        continue;
      };
      if let Some(rewritten) = rewrite_to_absolute(&current, base) {
        prop.set_value(jsonc_parser::cst::CstInputValue::String(rewritten));
      }
    }
  }

  if let Some(prop) = config_obj.get("imports")
    && let Some(obj) = prop.object_value()
  {
    rewrite_specifier_map(&obj, original_config_url);
  }

  if let Some(prop) = config_obj.get("scopes")
    && let Some(scopes_obj) = prop.object_value()
  {
    for scope_prop in scopes_obj.properties() {
      // Rewrite the scope prefix key if it's a relative path. Scope prefixes
      // are joined with the config base URL via `Url::join` in
      // `import_map::parse_scopes_map_json`, so after copying the config the
      // same prefix would resolve against the install dir.
      if let Some(name) = scope_prop.name() {
        rewrite_string_key(&name, original_config_url);
      }
      // Recurse into the per-scope imports object.
      if let Some(inner_obj) = scope_prop.object_value() {
        rewrite_specifier_map(&inner_obj, original_config_url);
      }
    }
  }
}

/// After packages are installed (including postinstall scripts), check if the
/// npm bin entry resolves to a native binary on disk. Returns the absolute path
/// to the binary if so. This handles packages like `@anthropic-ai/claude-code`
/// that ship platform-specific native binaries via optional dependencies and a
/// postinstall script that copies the binary into the bin entry path.
fn resolve_native_binary_path(
  bin_name_and_url: &BinaryNameAndUrl,
  installation_dir: &Path,
) -> Option<PathBuf> {
  let npm_ref =
    NpmPackageReqReference::from_specifier(&bin_name_and_url.module_url)
      .ok()?;
  let pkg_name = &npm_ref.req().name;
  let node_modules_pkg_dir = installation_dir
    .join(format!(".{}", bin_name_and_url.config_dir_name()))
    .join("node_modules")
    .join(pkg_name.as_str());

  let sys = crate::sys::CliSys::default();
  let bin_path = if let Some(sub_path) = npm_ref.sub_path() {
    // Extra bin entries encode the script path as the sub_path.
    node_modules_pkg_dir.join(sub_path)
  } else {
    // Primary entry: resolve the bin script via package.json.
    let pkg_json = deno_package_json::PackageJson::load_from_path(
      &sys,
      None,
      &node_modules_pkg_dir.join("package.json"),
    )
    .ok()
    .flatten()?;
    let bins = pkg_json.resolve_bins().ok()?;
    let deno_package_json::PackageJsonBins::Bins(bins) = bins else {
      return None;
    };
    // For a string bin field, `resolve_bins` keys the entry by the
    // package's default bin name, which may not match `--name` overrides.
    // Fall back to the sole entry when there's only one bin.
    bins
      .get(&bin_name_and_url.name)
      .or_else(|| {
        if bins.len() == 1 {
          bins.values().next()
        } else {
          None
        }
      })?
      .clone()
  };

  // Only treat the bin entry as a native binary if its magic bytes match
  // ELF/Mach-O/PE. `node_resolver::read_bin_value` returns `Executable` for
  // any file whose first line isn't a recognized npx-style shebang (e.g.
  // `#!/usr/bin/env deno` scripts) — which would incorrectly cause us to
  // generate an `exec`-the-file shim instead of a `deno run` shim. On
  // Windows there is no shebang interpretation, so that breaks installs of
  // ordinary npm packages whose bin scripts use a non-Node shebang.
  use std::io::Read;
  let mut file = std::fs::File::open(&bin_path).ok()?;
  let mut buf = [0u8; 4];
  if file.read(&mut buf).ok()? < 4 {
    return None;
  }
  if node_resolver::is_binary(&buf) {
    Some(bin_path)
  } else {
    None
  }
}

fn create_install_shim(
  bin_name_and_url: &BinaryNameAndUrl,
  cwd: &Path,
  flags: &Flags,
  install_flags_global: &InstallFlagsGlobal,
) -> Result<(), AnyError> {
  let mut shim_data =
    resolve_shim_data(bin_name_and_url, cwd, flags, install_flags_global)?;

  // Check if the bin entry is a native binary (e.g. Node SEA or platform-specific
  // binary installed via postinstall). If so, the shim should exec it directly.
  shim_data.native_binary_path =
    resolve_native_binary_path(bin_name_and_url, &shim_data.installation_dir);

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
    native_binary_path: None,
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
  /// If set, the bin entry is a native binary and the shim should exec it
  /// directly instead of wrapping with `deno run`.
  native_binary_path: Option<PathBuf>,
}

#[cfg(windows)]
/// On Windows, 2 files are generated.
/// One compatible with cmd & powershell with a .cmd extension
/// A second compatible with git bash / MINGW64
/// Generate batch script to satisfy that.
fn generate_executable_file(shim_data: &ShimData) -> Result<(), AnyError> {
  if let Some(native_path) = &shim_data.native_binary_path {
    // Native binary: exec it directly
    let native_display = native_path.display();
    let template =
      format!("% generated by deno install %\n@\"{native_display}\" %*\n",);
    let mut file = File::create(&shim_data.file_path)?;
    file.write_all(template.as_bytes())?;

    let template = format!(
      r#"#!/bin/sh
# generated by deno install
exec "{native_display}" "$@"
"#,
    );
    let mut file = File::create(shim_data.file_path.with_extension(""))?;
    file.write_all(template.as_bytes())?;
  } else {
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
  }
  Ok(())
}

#[cfg(not(windows))]
fn generate_executable_file(shim_data: &ShimData) -> Result<(), AnyError> {
  let template = if let Some(native_path) = &shim_data.native_binary_path {
    let path_str = posix_shell_escape(&native_path.to_string_lossy());
    format!(
      r#"#!/bin/sh
# generated by deno install
exec {} "$@"
"#,
      path_str,
    )
  } else {
    let args: Vec<String> = shim_data
      .args
      .iter()
      .map(|arg| posix_shell_escape(arg))
      .collect();
    format!(
      r#"#!/bin/sh
# generated by deno install
exec deno {} "$@"
"#,
      args.join(" "),
    )
  };
  let mut file = File::create(&shim_data.file_path)?;
  file.write_all(template.as_bytes())?;
  let _metadata = fs::metadata(&shim_data.file_path)?;
  let mut permissions = _metadata.permissions();
  permissions.set_mode(0o755);
  fs::set_permissions(&shim_data.file_path, permissions)?;
  Ok(())
}

#[cfg(not(windows))]
fn posix_shell_escape(arg: &str) -> String {
  if !arg.is_empty()
    && arg.chars().all(|ch| {
      matches!(
        ch,
        'a'..='z'
          | 'A'..='Z'
          | '0'..='9'
          | '-'
          | '_'
          | '='
          | '/'
          | ','
          | '.'
          | '+'
      )
    })
  {
    return arg.to_string();
  }

  let mut escaped = String::with_capacity(arg.len() + 2);
  escaped.push('\'');
  for ch in arg.chars() {
    match ch {
      '\'' | '!' => {
        escaped.push_str("'\\");
        escaped.push(ch);
        escaped.push('\'');
      }
      _ => escaped.push(ch),
    }
  }
  escaped.push('\'');
  escaped
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

/// A `deno.lock` fetched from JSR, ready to be written into a global install
/// directory.
struct FetchedJsrLockfile {
  content: String,
  /// Import map entries (bare package name -> `jsr:`/`npm:` specifier) for the
  /// workspace dependency roots recorded in the published lockfile.
  workspace_imports: BTreeMap<String, String>,
}

impl JsrLockfileFetcher<'_> {
  async fn fetch_lockfile(
    &self,
    module_url: &Url,
  ) -> Option<FetchedJsrLockfile> {
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
    let workspace_imports = lockfile
      .workspace_dep_reqs()
      .map(|req| (req.req.name.to_string(), req.to_string()))
      .collect();
    Some(FetchedJsrLockfile {
      content: lockfile.as_json_string(),
      workspace_imports,
    })
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
      &cwd,
      &installation_dir,
      None,
      install_flags_global.force,
      &[],
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
  async fn install_force_regenerates_lockfile() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin");
    std::fs::create_dir(&bin_dir).unwrap();

    // initial install creates the config dir
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

    // simulate a stale auto-generated lockfile from a prior install
    let config_dir = bin_dir.join(".echo_test");
    let lockfile_path = config_dir.join("deno.lock");
    let stale_lockfile =
      r#"{"version":"5","specifiers":{"npm:cowsay@*":"1.0.0"}}"#;
    fs::write(&lockfile_path, stale_lockfile).unwrap();
    assert!(lockfile_path.exists());

    // reinstall with --force; stale lockfile should be removed
    create_install_shim(
      &Flags::default(),
      InstallFlagsGlobal {
        module_urls: vec!["http://localhost:4545/echo.ts".to_string()],
        args: vec![],
        name: Some("echo_test".to_string()),
        root: Some(temp_dir.path().to_string()),
        force: true,
        compile: false,
      },
    )
    .await
    .unwrap();

    let post_force_content = fs::read_to_string(&lockfile_path).ok();
    assert_ne!(post_force_content.as_deref(), Some(stale_lockfile));
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

    // create a shim plus its extra and hidden per-command files, mirroring
    // what install produces
    let create_shim = |name: &str| {
      let mut file_path = bin_dir.join(name);
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

      (file_path, shim_path)
    };

    let (mut file_path, shim_path) = create_shim("echo_test");
    let (mut second_file_path, second_shim_path) =
      create_shim("second_echo_test");

    // uninstall removes multiple packages at once
    uninstall(
      Default::default(),
      UninstallFlags {
        kind: UninstallKind::Global(UninstallFlagsGlobal {
          packages: vec![
            "echo_test".to_string(),
            "second_echo_test".to_string(),
          ],
          root: Some(temp_dir.path().to_string()),
        }),
      },
    )
    .await
    .unwrap();

    for (file_path, shim_path) in [
      (&file_path, &shim_path),
      (&second_file_path, &second_shim_path),
    ] {
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
    }

    if cfg!(windows) {
      file_path = file_path.with_extension("cmd");
      assert!(!file_path.exists());

      second_file_path = second_file_path.with_extension("cmd");
      assert!(!second_file_path.exists());
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
      replace_registry_host: Default::default(),
      min_release_age_days: None,
      trust_policy: Default::default(),
      trust_policy_ignore_after_minutes: None,
      trust_policy_exclude: Vec::new(),
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

  #[test]
  fn native_binary_shim_for_npm_package() {
    // Set up a temp directory mimicking the global install layout:
    //   <root>/bin/.<name>/node_modules/<pkg>/package.json
    //   <root>/bin/.<name>/node_modules/<pkg>/bin/tool.exe  (Mach-O magic)
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin").to_path_buf();
    let config_dir = bin_dir.join(".mytool");
    let pkg_dir = config_dir.join("node_modules").join("mytool");
    let bin_sub = pkg_dir.join("bin");
    std::fs::create_dir_all(&bin_sub).unwrap();

    // Write a package.json with a bin entry pointing to a binary
    std::fs::write(
      pkg_dir.join("package.json"),
      r#"{"name": "mytool", "bin": {"mytool": "bin/tool.exe"}}"#,
    )
    .unwrap();

    // Write a fake Mach-O 64-bit binary (magic bytes: 0xcffaedfe in LE = 0xfeedfacf)
    let mut macho_bytes = vec![0xcf, 0xfa, 0xed, 0xfe];
    macho_bytes.extend_from_slice(&[0u8; 100]); // pad
    std::fs::write(bin_sub.join("tool.exe"), &macho_bytes).unwrap();

    // Write the config dir's deno.json so the directory exists as expected
    std::fs::write(config_dir.join("deno.json"), "{}").unwrap();

    let bin_name_and_url = BinaryNameAndUrl {
      name: "mytool".to_string(),
      module_url: Url::parse("npm:mytool@1.0.0").unwrap(),
      config_name: None,
    };

    let result = super::resolve_native_binary_path(&bin_name_and_url, &bin_dir);
    assert!(result.is_some(), "should detect Mach-O binary");
    assert!(
      result.unwrap().ends_with("bin/tool.exe"),
      "should return path to the binary"
    );
  }

  #[test]
  fn native_binary_not_detected_for_js_file() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin").to_path_buf();
    let config_dir = bin_dir.join(".mytool");
    let pkg_dir = config_dir.join("node_modules").join("mytool");
    std::fs::create_dir_all(&pkg_dir).unwrap();

    std::fs::write(
      pkg_dir.join("package.json"),
      r#"{"name": "mytool", "bin": {"mytool": "cli.js"}}"#,
    )
    .unwrap();
    std::fs::write(
      pkg_dir.join("cli.js"),
      "#!/usr/bin/env node\nconsole.log('hello');",
    )
    .unwrap();

    let bin_name_and_url = BinaryNameAndUrl {
      name: "mytool".to_string(),
      module_url: Url::parse("npm:mytool@1.0.0").unwrap(),
      config_name: None,
    };

    let result = super::resolve_native_binary_path(&bin_name_and_url, &bin_dir);
    assert!(
      result.is_none(),
      "should not detect JS file as native binary"
    );
  }

  #[test]
  fn native_binary_not_detected_for_non_node_shebang_js() {
    // Regression: a JS bin script with a non-Node shebang (e.g.
    // `#!/usr/bin/env deno`) was being misclassified as a native binary,
    // which broke `deno install -g` on Windows since the generated shim
    // execed the .js file directly instead of running `deno run`.
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join("bin").to_path_buf();
    let config_dir = bin_dir.join(".mytool");
    let pkg_dir = config_dir.join("node_modules").join("mytool");
    std::fs::create_dir_all(&pkg_dir).unwrap();

    std::fs::write(
      pkg_dir.join("package.json"),
      r#"{"name": "mytool", "bin": {"mytool": "./main.js"}}"#,
    )
    .unwrap();
    std::fs::write(
      pkg_dir.join("main.js"),
      "#!/usr/bin/env deno\nconsole.log('hello');",
    )
    .unwrap();

    let bin_name_and_url = BinaryNameAndUrl {
      name: "mytool".to_string(),
      module_url: Url::parse("npm:mytool@1.0.0").unwrap(),
      config_name: None,
    };

    let result = super::resolve_native_binary_path(&bin_name_and_url, &bin_dir);
    assert!(
      result.is_none(),
      "JS file with non-Node shebang must not be classified as native binary"
    );
  }

  #[cfg(not(windows))]
  #[test]
  fn posix_shell_escape_matches_shim_needs() {
    assert_eq!(
      super::posix_shell_escape("/install/dir/.mytool/bin/tool.exe"),
      "/install/dir/.mytool/bin/tool.exe"
    );
    assert_eq!(super::posix_shell_escape("two words"), "'two words'");
    assert_eq!(super::posix_shell_escape("it's"), "'it'\\''s'");
    assert_eq!(
      super::posix_shell_escape("$(echo hi);&|<>*?[]"),
      "'$(echo hi);&|<>*?[]'"
    );
    assert_eq!(super::posix_shell_escape("%PATH%/100%"), "'%PATH%/100%'");
    assert_eq!(super::posix_shell_escape(""), "''");
  }

  #[cfg(not(windows))]
  #[test]
  fn generate_shim_quotes_shell_sensitive_args() {
    let temp_dir = TempDir::new();
    let file_path = temp_dir.path().join("mytool").to_path_buf();
    let shim_data = ShimData {
      installation_dir: temp_dir.path().to_path_buf(),
      file_path: file_path.clone(),
      args: vec![
        "run".to_string(),
        "--config".to_string(),
        "path with spaces/deno.json".to_string(),
        "https://example.com/it's.ts".to_string(),
        "$(echo hi);&|<>*?[]".to_string(),
        "%PATH%/100%".to_string(),
        "".to_string(),
      ],
      native_binary_path: None,
    };
    super::generate_executable_file(&shim_data).unwrap();
    let content = fs::read_to_string(&file_path).unwrap();
    assert!(
      content.contains(
        "exec deno run --config 'path with spaces/deno.json' \
         'https://example.com/it'\\''s.ts' '$(echo hi);&|<>*?[]' \
         '%PATH%/100%' '' \"$@\""
      ),
      "shim should quote shell-sensitive args, got: {content}"
    );
  }

  #[cfg(not(windows))]
  #[test]
  fn generate_shim_for_normal_native_binary_path_stays_plain() {
    let temp_dir = TempDir::new();
    let file_path = temp_dir.path().join("mytool").to_path_buf();
    let shim_data = ShimData {
      installation_dir: temp_dir.path().to_path_buf(),
      file_path: file_path.clone(),
      args: vec!["run".to_string(), "npm:mytool".to_string()],
      native_binary_path: Some(PathBuf::from(
        "/install/dir/.mytool/node_modules/mytool/bin/tool.exe",
      )),
    };
    super::generate_executable_file(&shim_data).unwrap();
    let content = fs::read_to_string(&file_path).unwrap();
    assert!(
      content
        .contains("exec /install/dir/.mytool/node_modules/mytool/bin/tool.exe"),
      "normal native binary paths should not need quotes, got: {content}"
    );
  }

  #[cfg(not(windows))]
  #[test]
  fn generate_shim_for_native_binary() {
    let temp_dir = TempDir::new();
    let file_path = temp_dir.path().join("mytool").to_path_buf();
    let shim_data = ShimData {
      installation_dir: temp_dir.path().to_path_buf(),
      file_path: file_path.clone(),
      args: vec!["run".to_string(), "npm:mytool".to_string()],
      native_binary_path: Some(PathBuf::from(
        "/install/dir/.mytool/node_modules/mytool/bin/tool.exe",
      )),
    };
    super::generate_executable_file(&shim_data).unwrap();
    let content = fs::read_to_string(&file_path).unwrap();
    assert!(
      content
        .contains("exec /install/dir/.mytool/node_modules/mytool/bin/tool.exe"),
      "shim should exec native binary directly, got: {content}"
    );
    assert!(
      !content.contains("exec deno"),
      "shim should not exec deno, got: {content}"
    );
  }

  #[cfg(not(windows))]
  #[test]
  fn generate_shim_for_js_module() {
    let temp_dir = TempDir::new();
    let file_path = temp_dir.path().join("mytool").to_path_buf();
    let shim_data = ShimData {
      installation_dir: temp_dir.path().to_path_buf(),
      file_path: file_path.clone(),
      args: vec!["run".to_string(), "npm:mytool".to_string()],
      native_binary_path: None,
    };
    super::generate_executable_file(&shim_data).unwrap();
    let content = fs::read_to_string(&file_path).unwrap();
    assert!(
      content.contains("exec deno run"),
      "shim should use deno run, got: {content}"
    );
  }
}
