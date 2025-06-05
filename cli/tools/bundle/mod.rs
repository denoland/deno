// Copyright 2018-2025 the Deno authors. MIT license.

mod esbuild;

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_cache_dir::GlobalOrLocalHttpCache;
use deno_config::deno_json::TsTypeLib;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::url::Url;
use deno_core::ModuleLoader;
use deno_error::JsError;
use deno_graph::Position;
use deno_lib::worker::ModuleLoaderFactory;
use deno_resolver::npm::managed::ResolvePkgFolderFromDenoModuleError;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageReqReference;
use esbuild_client::protocol;
use esbuild_client::EsbuildFlagsBuilder;
use esbuild_client::EsbuildService;
use indexmap::IndexMap;
use node_resolver::errors::PackageSubpathResolveError;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use regex::Regex;
use sys_traits::EnvCurrentDir;

use crate::args::BundleFlags;
use crate::args::BundleFormat;
use crate::args::Flags;
use crate::args::PackageHandling;
use crate::factory::CliFactory;
use crate::graph_container::MainModuleGraphContainer;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::module_loader::ModuleLoadPreparer;
use crate::module_loader::PrepareModuleLoadOptions;
use crate::node::CliNodeResolver;
use crate::npm::CliNpmResolver;
use crate::resolver::CliNpmReqResolver;
use crate::resolver::CliResolver;
use crate::sys::CliSys;

/// Given a set of pattern indicating files to mark as external,
/// return a regex that matches any of those patterns.
///
/// For instance given, `--external="*.node" --external="*.wasm"`, the regex will match
/// any path that ends with `.node` or `.wasm`.
pub fn externals_regex(external: &[String]) -> Regex {
  let mut regex_str = String::new();
  for (i, e) in external.iter().enumerate() {
    if i > 0 {
      regex_str.push('|');
    }
    regex_str.push_str("(^");
    if e.starts_with("/") {
      regex_str.push_str(".*");
    }
    regex_str.push_str(&regex::escape(e).replace("\\*", ".*"));
    regex_str.push(')');
  }
  regex::Regex::new(&regex_str).unwrap()
}

pub async fn bundle(
  flags: Arc<Flags>,
  bundle_flags: BundleFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);

  let installer_factory = factory.npm_installer_factory()?;
  let npmrc = factory.npmrc()?;
  let deno_dir = factory.deno_dir()?;
  let resolver_factory = factory.resolver_factory()?;
  let workspace_factory = resolver_factory.workspace_factory();
  let npm_registry_info = installer_factory.registry_info_provider()?;
  let esbuild_path = esbuild::ensure_esbuild(
    deno_dir,
    npmrc,
    npm_registry_info,
    workspace_factory.workspace_npm_patch_packages()?,
    installer_factory.tarball_cache()?,
    factory.npm_cache()?,
  )
  .await?;
  let resolver = factory.resolver().await?.clone();
  let module_load_preparer = factory.module_load_preparer().await?.clone();
  let root_permissions = factory.root_permissions_container()?;
  let npm_resolver = factory.npm_resolver().await?.clone();
  let node_resolver = factory.node_resolver().await?.clone();
  let cli_options = factory.cli_options()?;
  let module_loader = factory
    .create_module_loader_factory()
    .await?
    .create_for_main(root_permissions.clone())
    .module_loader;
  let sys = factory.sys();
  let init_cwd = cli_options.initial_cwd().canonicalize()?;
  let npm_req_resolver = factory.npm_req_resolver()?;

  #[allow(clippy::arc_with_non_send_sync)]
  let plugin_handler = Arc::new(DenoPluginHandler {
    resolver: resolver.clone(),
    module_load_preparer,
    module_graph_container: factory
      .main_module_graph_container()
      .await?
      .clone(),
    permissions: root_permissions.clone(),
    npm_req_resolver: npm_req_resolver.clone(),
    npm_resolver: npm_resolver.clone(),
    node_resolver: node_resolver.clone(),
    http_cache: factory.http_cache()?.clone(),
    module_loader: module_loader.clone(),
    // TODO(nathanwhit): look at the external patterns to give diagnostics for probably incorrect patterns
    externals_regex: if bundle_flags.external.is_empty() {
      None
    } else {
      Some(externals_regex(&bundle_flags.external))
    },
  });
  let start = std::time::Instant::now();

  let entrypoint = bundle_flags
    .entrypoints
    .first()
    .iter()
    .map(|e| resolve_url_or_path(e, &init_cwd).unwrap())
    .collect::<Vec<_>>();
  let resolved = {
    let mut resolved = vec![];
    let init_cwd_url = Url::from_directory_path(&init_cwd).unwrap();
    for e in &entrypoint {
      let r = resolver
        .resolve(
          e.as_str(),
          &init_cwd_url,
          Position::new(0, 0),
          ResolutionMode::Import,
          NodeResolutionKind::Execution,
        )
        .unwrap();
      resolved.push(r);
    }
    resolved
  };
  let _ = plugin_handler.prepare_module_load(&resolved).await;

  let roots = resolved
    .into_iter()
    .map(|url| {
      if let Ok(v) = NpmPackageReqReference::from_specifier(&url) {
        let referrer =
          ModuleSpecifier::from_directory_path(sys.env_current_dir().unwrap())
            .unwrap();
        let package_folder = npm_resolver
          .resolve_pkg_folder_from_deno_module_req(v.req(), &referrer)
          .unwrap();
        let main_module = node_resolver
          .resolve_binary_export(&package_folder, v.sub_path())
          .unwrap();
        Url::from_file_path(&main_module).unwrap()
      } else {
        url
      }
    })
    .collect::<Vec<_>>();
  let _ = plugin_handler.prepare_module_load(&roots).await;
  let esbuild = EsbuildService::new(
    esbuild_path,
    esbuild::ESBUILD_VERSION,
    plugin_handler.clone(),
  )
  .await
  .unwrap();
  let client = esbuild.client().clone();

  {
    tokio::spawn(async move {
      let res = esbuild.wait_for_exit().await;
      log::warn!("esbuild exited: {:?}", res);

      Ok::<(), AnyError>(())
    });
  }

  let mut builder = EsbuildFlagsBuilder::default();
  builder
    .bundle(bundle_flags.one_file)
    .minify(bundle_flags.minify)
    .splitting(bundle_flags.code_splitting)
    .external(bundle_flags.external.clone())
    .tree_shaking(true)
    .format(match bundle_flags.format {
      BundleFormat::Esm => esbuild_client::Format::Esm,
      BundleFormat::Cjs => esbuild_client::Format::Cjs,
      BundleFormat::Iife => esbuild_client::Format::Iife,
    })
    .packages(match bundle_flags.packages {
      PackageHandling::External => esbuild_client::PackagesHandling::External,
      PackageHandling::Bundle => esbuild_client::PackagesHandling::Bundle,
    });
  if let Some(outdir) = bundle_flags.output_dir.clone() {
    builder.outdir(outdir);
  } else if let Some(output_path) = bundle_flags.output_path.clone() {
    builder.outfile(output_path);
  }
  let flags = builder.build().unwrap();

  let entries = roots.into_iter().map(|e| ("".into(), e.into())).collect();

  let response = client
    .send_build_request(protocol::BuildRequest {
      entries,
      key: 0,
      flags: flags.to_flags(),
      write: true,
      stdin_contents: None.into(),
      stdin_resolve_dir: None.into(),
      abs_working_dir: init_cwd.to_string_lossy().to_string(),
      context: false,
      mangle_cache: None,
      node_paths: vec![],
      plugins: Some(vec![protocol::BuildPlugin {
        name: "deno".into(),
        on_start: false,
        on_end: false,
        on_resolve: (vec![protocol::OnResolveSetupOptions {
          id: 0,
          filter: ".*".into(),
          namespace: "".into(),
        }]),
        on_load: vec![protocol::OnLoadSetupOptions {
          id: 0,
          filter: ".*".into(),
          namespace: "".into(),
        }],
      }]),
    })
    .await
    .unwrap();

  for error in &response.errors {
    log::error!(
      "{}: {}",
      deno_terminal::colors::red("bundler error"),
      format_message(error)
    );
  }

  for warning in &response.warnings {
    log::warn!(
      "{}: {}",
      deno_terminal::colors::yellow("bundler warning"),
      format_message(warning)
    );
  }

  if let Some(stdout) = response.write_to_stdout {
    #[allow(clippy::print_stdout)]
    {
      println!("{}", String::from_utf8_lossy(&stdout));
    }
  } else if response.errors.is_empty() {
    if bundle_flags.output_dir.is_none()
      && std::env::var("NO_DENO_BUNDLE_HACK").is_err()
      && bundle_flags.output_path.is_some()
    {
      let out = bundle_flags.output_path.as_ref().unwrap();
      let contents = std::fs::read_to_string(out).unwrap();
      let contents = replace_require_shim(&contents);
      std::fs::write(out, contents).unwrap();
    }

    log::info!(
      "{}",
      deno_terminal::colors::green(format!(
        "bundled in {}",
        crate::display::human_elapsed(start.elapsed().as_millis()),
      ))
    );
  }

  Ok(())
}

// TODO(nathanwhit): MASSIVE HACK
// See tests::specs::bundle::requires_node_builtin for why this is needed.
// Without this hack, that test would fail with "Dynamic require of "util" is not supported"
fn replace_require_shim(contents: &str) -> String {
  contents.replace(
    r#"var __require = /* @__PURE__ */ ((x) => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, {
  get: (a, b) => (typeof require !== "undefined" ? require : a)[b]
}) : x)(function(x) {
  if (typeof require !== "undefined") return require.apply(this, arguments);
  throw Error('Dynamic require of "' + x + '" is not supported');
});"#,
    r#"import { createRequire } from "node:module";
var __require = createRequire(import.meta.url);
"#,
  )
}

fn format_message(message: &esbuild_client::protocol::Message) -> String {
  format!(
    "{}{}",
    message.text,
    if let Some(location) = &message.location {
      format!(
        "\n  at {} {}:{}",
        location.file, location.line, location.column
      )
    } else {
      String::new()
    }
  )
}
#[derive(Debug, thiserror::Error, JsError)]
#[class(generic)]
enum BundleError {
  #[error(transparent)]
  Resolver(#[from] deno_graph::source::ResolveError),
  #[error(transparent)]
  Url(#[from] deno_core::url::ParseError),
  #[error(transparent)]
  ResolveNpmPkg(#[from] ResolvePkgFolderFromDenoModuleError),
  #[error(transparent)]
  SubpathResolve(#[from] PackageSubpathResolveError),
  #[error(transparent)]
  PathToUrlError(#[from] deno_path_util::PathToUrlError),
  #[error(transparent)]
  UrlToPathError(#[from] deno_path_util::UrlToFilePathError),
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error(transparent)]
  ResolveUrlOrPathError(#[from] deno_path_util::ResolveUrlOrPathError),
  #[error(transparent)]
  PrepareModuleLoad(#[from] crate::module_loader::PrepareModuleLoadError),
  #[error(transparent)]
  ResolveReqWithSubPath(#[from] deno_resolver::npm::ResolveReqWithSubPathError),
  #[error(transparent)]
  PackageReqReferenceParse(
    #[from] deno_semver::package::PackageReqReferenceParseError,
  ),
  #[allow(dead_code)]
  #[error("Http cache error")]
  HttpCache,
}

struct DenoPluginHandler {
  resolver: Arc<CliResolver>,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  module_graph_container: Arc<MainModuleGraphContainer>,
  permissions: PermissionsContainer,
  npm_req_resolver: Arc<CliNpmReqResolver>,
  npm_resolver: CliNpmResolver,
  node_resolver: Arc<CliNodeResolver>,
  #[allow(dead_code)]
  http_cache: GlobalOrLocalHttpCache<CliSys>,
  module_loader: Rc<dyn ModuleLoader>,
  externals_regex: Option<Regex>,
}

#[async_trait::async_trait(?Send)]
impl esbuild_client::PluginHandler for DenoPluginHandler {
  async fn on_resolve(
    &self,
    args: esbuild_client::OnResolveArgs,
  ) -> Result<Option<esbuild_client::OnResolveResult>, AnyError> {
    log::debug!("{}: {args:?}", deno_terminal::colors::cyan("on_resolve"));
    if let Some(reg) = &self.externals_regex {
      if reg.is_match(&args.path) {
        return Ok(Some(esbuild_client::OnResolveResult {
          external: Some(true),
          path: Some(args.path),
          plugin_name: Some("deno".to_string()),
          plugin_data: None,
          ..Default::default()
        }));
      }
    }
    let result = self.bundle_resolve(
      &args.path,
      args.importer.as_deref(),
      args.resolve_dir.as_deref(),
      args.kind,
      args.with,
    )?;

    Ok(result.map(|r| {
      esbuild_client::OnResolveResult {
        namespace: if r.starts_with("jsr:")
          || r.starts_with("https:")
          || r.starts_with("http:")
        {
          Some("deno".into())
        } else {
          None
        },
        external: Some(
          r.starts_with("node:")
            || self
              .externals_regex
              .as_ref()
              .map(|reg| reg.is_match(&r))
              .unwrap_or(false),
        ),
        path: Some(r),
        plugin_name: Some("deno".to_string()),
        plugin_data: None,
        ..Default::default()
      }
    }))
  }

  async fn on_load(
    &self,
    args: esbuild_client::OnLoadArgs,
  ) -> Result<Option<esbuild_client::OnLoadResult>, AnyError> {
    let result = self.bundle_load(&args.path, "").await?;
    log::trace!(
      "{}: {:?}",
      deno_terminal::colors::magenta("on_load"),
      result.as_ref().map(|(code, loader)| format!(
        "{}: {:?}",
        String::from_utf8_lossy(code),
        loader
      ))
    );
    if let Some((code, loader)) = result {
      Ok(Some(esbuild_client::OnLoadResult {
        contents: Some(code),
        loader: Some(loader),
        ..Default::default()
      }))
    } else {
      Ok(None)
    }
  }

  async fn on_start(
    &self,
    _args: esbuild_client::OnStartArgs,
  ) -> Result<Option<esbuild_client::OnStartResult>, AnyError> {
    Ok(None)
  }
}

fn import_kind_to_resolution_mode(
  kind: esbuild_client::protocol::ImportKind,
) -> ResolutionMode {
  match kind {
    protocol::ImportKind::EntryPoint
    | protocol::ImportKind::ImportStatement
    | protocol::ImportKind::ComposesFrom
    | protocol::ImportKind::DynamicImport
    | protocol::ImportKind::ImportRule
    | protocol::ImportKind::UrlToken => ResolutionMode::Import,
    protocol::ImportKind::RequireCall
    | protocol::ImportKind::RequireResolve => ResolutionMode::Require,
  }
}

impl DenoPluginHandler {
  #[allow(dead_code)]
  fn get_final_path(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<String, AnyError> {
    match specifier.scheme() {
      "file" => {
        let path = specifier.to_file_path().unwrap();
        Ok(path.to_string_lossy().to_string())
      }
      "npm" => {
        let req_ref = NpmPackageReqReference::from_specifier(specifier)?;
        let path = self.npm_req_resolver.resolve_req_reference(
          &req_ref,
          specifier,
          ResolutionMode::Import,
          NodeResolutionKind::Execution,
        )?;
        Ok(path.to_string_lossy().to_string())
      }
      "https" | "http" => Ok(match &self.http_cache {
        GlobalOrLocalHttpCache::Global(global_http_cache) => global_http_cache
          .local_path_for_url(specifier)?
          .to_string_lossy()
          .to_string(),
        GlobalOrLocalHttpCache::Local(local_http_cache) => local_http_cache
          .local_path_for_url(specifier)?
          .ok_or(BundleError::HttpCache)?
          .to_string_lossy()
          .to_string(),
      }),
      _ => Ok(specifier.to_string()),
    }
  }
  fn bundle_resolve(
    &self,
    path: &str,
    importer: Option<&str>,
    resolve_dir: Option<&str>,
    kind: esbuild_client::protocol::ImportKind,
    // TODO: use this / store it for later usage when loading
    with: IndexMap<String, String>,
  ) -> Result<Option<String>, AnyError> {
    log::debug!(
      "bundle_resolve: {:?} {:?} {:?} {:?} {:?}",
      path,
      importer,
      resolve_dir,
      kind,
      with
    );
    let mut resolve_dir = resolve_dir.unwrap_or("").to_string();
    let resolver = self.resolver.clone();
    if !resolve_dir.ends_with(std::path::MAIN_SEPARATOR) {
      resolve_dir.push(std::path::MAIN_SEPARATOR);
    }
    let resolve_dir_path = Path::new(&resolve_dir);
    let mut referrer =
      resolve_url_or_path(importer.unwrap_or(""), resolve_dir_path)
        .unwrap_or_else(|_| {
          Url::from_directory_path(std::env::current_dir().unwrap()).unwrap()
        });
    if referrer.scheme() == "file" {
      let pth = referrer.to_file_path().unwrap();
      if (pth.is_dir()) && !pth.ends_with(std::path::MAIN_SEPARATOR_STR) {
        referrer.set_path(&format!(
          "{}{}",
          referrer.path(),
          std::path::MAIN_SEPARATOR
        ));
      }
    }

    log::debug!(
      "{}: {} {} {} {:?}",
      deno_terminal::colors::magenta("op_bundle_resolve"),
      path,
      resolve_dir,
      referrer,
      import_kind_to_resolution_mode(kind)
    );

    let graph = self.module_graph_container.graph();
    let result = resolver.resolve_with_graph(
      &graph,
      path,
      &referrer,
      Position::new(0, 0),
      import_kind_to_resolution_mode(kind),
      NodeResolutionKind::Execution,
    );

    log::debug!(
      "{}: {:?}",
      deno_terminal::colors::cyan("op_bundle_resolve result"),
      result
    );

    match result {
      Ok(specifier) => {
        return Ok(Some(file_path_or_url(&specifier)?));
      }
      Err(e) => {
        log::debug!("{}: {:?}", deno_terminal::colors::red("error"), e);
        Err(BundleError::Resolver(e).into())
      }
    }
  }

  async fn prepare_module_load(
    &self,
    specifiers: &[ModuleSpecifier],
  ) -> Result<(), AnyError> {
    let mut graph_permit =
      self.module_graph_container.acquire_update_permit().await;
    let graph: &mut deno_graph::ModuleGraph = graph_permit.graph_mut();
    self
      .module_load_preparer
      .prepare_module_load(
        graph,
        specifiers,
        PrepareModuleLoadOptions {
          is_dynamic: false,
          lib: TsTypeLib::default(),
          permissions: self.permissions.clone(),
          ext_overwrite: None,
          allow_unknown_media_types: false,
          skip_graph_roots_validation: true,
        },
      )
      .await?;
    graph_permit.commit();
    Ok(())
  }

  async fn bundle_load(
    &self,
    specifier: &str,
    resolve_dir: &str,
  ) -> Result<Option<(Vec<u8>, esbuild_client::BuiltinLoader)>, AnyError> {
    log::debug!(
      "{}: {:?} {:?}",
      deno_terminal::colors::magenta("bundle_load"),
      specifier,
      resolve_dir
    );

    let resolve_dir = Path::new(&resolve_dir);
    let specifier = deno_core::resolve_url_or_path(specifier, resolve_dir)?;

    let (specifier, loader) = if let Some((specifier, loader)) =
      self.specifier_and_type_from_graph(&specifier)?
    {
      (specifier, loader)
    } else {
      log::debug!(
        "{}: no specifier and type from graph for {}",
        deno_terminal::colors::yellow("warn"),
        specifier
      );

      let (media_type, _) =
        deno_media_type::resolve_media_type_and_charset_from_content_type(
          &specifier, None,
        );
      if media_type == deno_media_type::MediaType::Unknown {
        return Ok(None);
      }
      (specifier, media_type_to_loader(media_type))
    };
    let loaded = self.module_loader.load(
      &specifier,
      None,
      false,
      deno_core::RequestedModuleType::None,
    );

    match loaded {
      deno_core::ModuleLoadResponse::Sync(module_source) => {
        Ok(Some((module_source?.code.as_bytes().to_vec(), loader)))
      }
      deno_core::ModuleLoadResponse::Async(pin) => {
        let pin = pin.await?;
        Ok(Some((pin.code.as_bytes().to_vec(), loader)))
      }
    }
  }

  fn specifier_and_type_from_graph(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<(ModuleSpecifier, esbuild_client::BuiltinLoader)>, AnyError>
  {
    let graph = self.module_graph_container.graph();
    let Some(module) = graph.get(specifier) else {
      return Ok(None);
    };
    let (specifier, loader) = match module {
      deno_graph::Module::Js(js_module) => (
        js_module.specifier.clone(),
        media_type_to_loader(js_module.media_type),
      ),
      deno_graph::Module::Json(json_module) => (
        json_module.specifier.clone(),
        esbuild_client::BuiltinLoader::Json,
      ),
      deno_graph::Module::Wasm(_) => todo!(),
      deno_graph::Module::Npm(module) => {
        let package_folder = self
          .npm_resolver
          .as_managed()
          .unwrap() // byonm won't create a Module::Npm
          .resolve_pkg_folder_from_deno_module(module.nv_reference.nv())?;
        let path = self
          .node_resolver
          .resolve_package_subpath_from_deno_module(
            &package_folder,
            module.nv_reference.sub_path(),
            None,
            ResolutionMode::Import,
            NodeResolutionKind::Execution,
          )?;
        let url = path.clone().into_url()?;
        let (media_type, _charset) =
          deno_media_type::resolve_media_type_and_charset_from_content_type(
            &url, None,
          );
        (url, media_type_to_loader(media_type))
      }
      deno_graph::Module::Node(_) => {
        return Ok(None);
      }
      deno_graph::Module::External(_) => {
        return Ok(None);
      }
    };
    Ok(Some((specifier, loader)))
  }
}

fn file_path_or_url(url: &Url) -> Result<String, AnyError> {
  if url.scheme() == "file" {
    Ok(
      deno_path_util::url_to_file_path(url)?
        .to_string_lossy()
        .into(),
    )
  } else {
    Ok(url.to_string())
  }
}
fn media_type_to_loader(
  media_type: deno_media_type::MediaType,
) -> esbuild_client::BuiltinLoader {
  use deno_ast::MediaType::*;
  match media_type {
    JavaScript | Cjs | Mjs | Mts => esbuild_client::BuiltinLoader::Js,
    TypeScript | Cts | Dts | Dmts | Dcts => esbuild_client::BuiltinLoader::Ts,
    Jsx | Tsx => esbuild_client::BuiltinLoader::Jsx,
    Css => esbuild_client::BuiltinLoader::Css,
    Json => esbuild_client::BuiltinLoader::Json,
    SourceMap => esbuild_client::BuiltinLoader::Text,
    Html => esbuild_client::BuiltinLoader::Text,
    Sql => esbuild_client::BuiltinLoader::Text,
    Wasm => esbuild_client::BuiltinLoader::Binary,
    Unknown => esbuild_client::BuiltinLoader::Binary,
    // _ => esbuild_client::BuiltinLoader::External,
  }
}
