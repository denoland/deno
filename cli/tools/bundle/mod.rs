use std::path::Path;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_cache_dir::GlobalOrLocalHttpCache;
use deno_config::deno_json::TsTypeLib;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::url::Url;
use deno_error::JsError;
use deno_resolver::npm::managed::ResolvePkgFolderFromDenoModuleError;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageReqReference;
use esbuild_rs::protocol;
use esbuild_rs::EsbuildFlagsBuilder;
use esbuild_rs::EsbuildService;
use indexmap::IndexMap;
use node_resolver::errors::PackageSubpathResolveError;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;

use crate::args::BundleFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::graph_container::MainModuleGraphContainer;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::module_loader::ModuleLoadPreparer;
use crate::module_loader::PrepareModuleLoadOptions;
use crate::node::CliNodeResolver;
use crate::npm::CliNpmResolver;
use crate::resolver::CliDenoResolver;
use crate::resolver::CliNpmReqResolver;
use crate::sys::CliSys;

pub async fn bundle(
  flags: Arc<Flags>,
  mut bundle_flags: BundleFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);

  let resolver = factory.deno_resolver().await?.clone();
  let module_load_preparer = factory.module_load_preparer().await?.clone();
  let module_graph_container =
    factory.main_module_graph_container().await?.clone();
  let root_permissions = factory.root_permissions_container()?;
  let npm_req_resolver = factory.npm_req_resolver()?.clone();
  let npm_resolver = factory.npm_resolver().await?.clone();
  let node_resolver = factory.node_resolver().await?.clone();
  let cli_options = factory.cli_options()?;
  let http_cache = factory.http_cache()?.clone();

  let init_cwd = cli_options.initial_cwd().canonicalize()?;

  let path =
    "/Users/nathanwhit/Library/Caches/esbuild/bin/@esbuild-darwin-arm64@0.25.4";
  let path = Path::new(&path);

  let plugin_handler = Arc::new(DenoPluginHandler {
    resolver,
    module_load_preparer,
    module_graph_container,
    permissions: root_permissions.clone(),
    npm_req_resolver,
    npm_resolver,
    node_resolver,
    http_cache,
  });
  let esbuild = EsbuildService::new(path, "0.25.4", plugin_handler.clone())
    .await
    .unwrap();
  let client = esbuild.client().clone();

  {
    tokio::spawn(async move {
      loop {
        tokio::select! {
            res = esbuild.wait_for_exit() => {
                eprintln!("esbuild exited: {:?}", res);
                break;
            }
        }
      }

      Ok::<(), AnyError>(())
    });
  }

  bundle_flags.external.push("*.node".into());

  let output_path = bundle_flags
    .output_path
    .unwrap_or_else(|| "./dist/bundled.js".to_string());
  let flags = EsbuildFlagsBuilder::default()
    .outfile(output_path)
    .bundle(true)
    .external(bundle_flags.external)
    .loader(
      [(".node".into(), esbuild_rs::BuiltinLoader::File)]
        .into_iter()
        .collect(),
    )
    // .outfile("./temp/mod.js".into())
    .build()
    .unwrap();
  let entrypoint = bundle_flags.entrypoint;

  let response = client
    .send_build_request(protocol::BuildRequest {
      entries: vec![("".into(), entrypoint.into())],
      key: 0,
      flags: flags.to_flags(),
      write: true,
      stdin_contents: None.into(),
      stdin_resolve_dir: None.into(),
      abs_working_dir: init_cwd.to_string_lossy().to_string(),
      context: false,
      mangle_cache: None.into(),
      node_paths: vec![],
      plugins: Some(vec![protocol::BuildPlugin {
        name: "test".into(),
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

  log::info!(
    "{}: {:?}",
    deno_terminal::colors::green("build response"),
    response
  );

  if let Some(stdout) = response.write_to_stdout {
    println!("{}", String::from_utf8_lossy(&stdout));
  }

  Ok(())
  // let plugin
}

#[derive(Debug, thiserror::Error, JsError)]
#[class(generic)]
enum BundleError {
  // #[error("Invalid import kind")]
  // InvalidImportKind,
  #[error(transparent)]
  Resolver(#[from] deno_resolver::DenoResolveError),
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
  #[error("Http cache error")]
  HttpCache,
}

struct DenoPluginHandler {
  resolver: Arc<CliDenoResolver>,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  module_graph_container: Arc<MainModuleGraphContainer>,
  permissions: PermissionsContainer,
  npm_req_resolver: Arc<CliNpmReqResolver>,
  npm_resolver: CliNpmResolver,
  node_resolver: Arc<CliNodeResolver>,
  #[allow(dead_code)]
  http_cache: GlobalOrLocalHttpCache<CliSys>,
}

#[async_trait::async_trait(?Send)]
impl esbuild_rs::PluginHandler for DenoPluginHandler {
  async fn on_resolve(
    &self,
    args: esbuild_rs::OnResolveArgs,
  ) -> Result<Option<esbuild_rs::OnResolveResult>, AnyError> {
    log::debug!("{}: {args:?}", deno_terminal::colors::cyan("on_resolve"));
    let result = self
      .bundle_resolve(
        &args.path,
        args.importer.as_deref(),
        args.resolve_dir.as_deref(),
        args.kind,
        args.with,
      )
      .await?;

    Ok(result.map(|r| esbuild_rs::OnResolveResult {
      namespace: if r.starts_with("jsr:")
        || r.starts_with("https:")
        || r.starts_with("http:")
      {
        Some("deno".into())
      } else {
        None
      },
      external: Some(r.starts_with("node:")),
      path: Some(r),
      plugin_name: Some("deno".to_string()),
      plugin_data: None.into(),
      ..Default::default()
    }))
  }

  async fn on_load(
    &self,
    args: esbuild_rs::OnLoadArgs,
  ) -> Result<Option<esbuild_rs::OnLoadResult>, AnyError> {
    let result = self.bundle_load(&args.path, "").await?;
    if let Some((code, loader)) = result {
      Ok(Some(esbuild_rs::OnLoadResult {
        contents: Some(code),
        loader: Some(loader),
        ..Default::default()
      }))
    } else {
      Ok(None)
    }
  }
}

fn import_kind_to_resolution_mode(
  kind: esbuild_rs::protocol::ImportKind,
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
  async fn bundle_resolve(
    &self,
    path: &str,
    importer: Option<&str>,
    resolve_dir: Option<&str>,
    kind: esbuild_rs::protocol::ImportKind,
    with: IndexMap<String, String>,
  ) -> Result<Option<String>, AnyError> {
    log::debug!(
      "op_bundle_resolve: {:?} {:?} {:?} {:?} {:?}",
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
      resolve_url_or_path(&importer.unwrap_or(""), resolve_dir_path)
        .unwrap_or_else(|_| {
          Url::from_directory_path(std::env::current_dir().unwrap()).unwrap()
        });
    if referrer.scheme() == "file" {
      let pth = referrer.to_file_path().unwrap();
      if ((&pth).is_dir()) && !pth.ends_with(std::path::MAIN_SEPARATOR_STR) {
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
    let result = resolver.resolve(
      &path,
      &referrer,
      import_kind_to_resolution_mode(kind),
      node_resolver::NodeResolutionKind::Execution,
    );

    log::debug!(
      "{}: {:?}",
      deno_terminal::colors::cyan("op_bundle_resolve result"),
      result
    );

    // eprintln!("op_bundle_resolve result: {:?}", result);
    // return error, but don't error out. defer until after all plugins have run. if still
    // nothing resolved it, then error out with this error.
    match result {
      Ok(result) => {
        log::debug!(
          "{}: {:?}",
          deno_terminal::colors::cyan("preparing module load"),
          result.url
        );
        self.prepare_module_load(result.url.clone()).await?;
        log::debug!(
          "{}: {:?}",
          deno_terminal::colors::green("prepared module load"),
          result.url
        );

        let graph = self.module_graph_container.graph();
        let module = graph.get(&result.url);
        if let Some(module) = module {
          log::debug!(
            "{}: {} -> {}",
            deno_terminal::colors::cyan("module"),
            result.url,
            module.specifier()
          );
          let specifier = match module {
            deno_graph::Module::Npm(_) => {
              let req_ref =
                NpmPackageReqReference::from_specifier(&result.url)?;
              return Ok(Some(file_path_or_url(
                &self
                  .npm_req_resolver
                  .resolve_req_reference(
                    &req_ref,
                    &referrer,
                    ResolutionMode::Import,
                    NodeResolutionKind::Execution,
                  )?
                  .into_url()?,
              )));
            }
            _ => module.specifier().clone(),
          };
          return Ok(Some(file_path_or_url(&specifier)));
        }

        if result.url.scheme() == "npm" {
          let req_ref = NpmPackageReqReference::from_specifier(&result.url)?;
          Ok(Some(file_path_or_url(
            &self
              .npm_req_resolver
              .resolve_req_reference(
                &req_ref,
                &referrer,
                ResolutionMode::Import,
                NodeResolutionKind::Execution,
              )?
              .into_url()?,
          )))
        } else {
          Ok(Some(file_path_or_url(&result.url)))
        }
      }
      Err(e) => {
        log::error!("{}: {:?}", deno_terminal::colors::red("error"), e);
        Err(BundleError::Resolver(e).into())
      }
    }
  }

  async fn prepare_module_load(
    &self,
    specifier: ModuleSpecifier,
  ) -> Result<(), AnyError> {
    let mut graph_permit =
      self.module_graph_container.acquire_update_permit().await;
    let graph: &mut deno_graph::ModuleGraph = graph_permit.graph_mut();
    // eprintln!("about to prepare module load");
    let prepared = self
      .module_load_preparer
      .prepare_module_load(
        graph,
        &[specifier],
        PrepareModuleLoadOptions {
          is_dynamic: false,
          lib: TsTypeLib::default(),
          permissions: self.permissions.clone(),
          ext_overwrite: None,
          allow_unknown_media_types: false,
        },
      )
      .await
      .inspect_err(|e| {
        // eprintln!(
        //   "{}: error preparing module load: {:?}",
        //   deno_terminal::colors::red("ERROR"),
        //   e
        // );
      })?;
    // eprintln!("prepared module load");
    graph_permit.commit();
    Ok(())
  }

  async fn bundle_load(
    &self,
    specifier: &str,
    resolve_dir: &str,
  ) -> Result<Option<(Vec<u8>, esbuild_rs::BuiltinLoader)>, AnyError> {
    let module_load_preparer = self.module_load_preparer.clone();
    let module_graph_container = self.module_graph_container.clone();
    let resolve_dir = Path::new(&resolve_dir);
    let specifier = deno_core::resolve_url_or_path(&specifier, resolve_dir)?;

    let npm_resolver = self.npm_resolver.clone();
    let node_resolver = self.node_resolver.clone();
    {
      let mut graph_permit =
        module_graph_container.acquire_update_permit().await;
      let graph: &mut deno_graph::ModuleGraph = graph_permit.graph_mut();
      // eprintln!("about to prepare module load");
      let prepared = module_load_preparer
        .prepare_module_load(
          graph,
          &[specifier.clone()],
          PrepareModuleLoadOptions {
            is_dynamic: false,
            lib: TsTypeLib::default(),
            permissions: self.permissions.clone(),
            ext_overwrite: None,
            allow_unknown_media_types: false,
          },
        )
        .await
        .inspect_err(|e| {
          // eprintln!(
          //   "{}: error preparing module load: {:?}",
          //   deno_terminal::colors::red("ERROR"),
          //   e
          // );
        })?;
      // eprintln!("prepared module load");
      graph_permit.commit();
    }
    let graph = module_graph_container.graph();
    let module = graph.get(&specifier).unwrap();
    let (code, loader) = match module {
      deno_graph::Module::Js(js_module) => (
        js_module.source.as_bytes().to_vec(),
        media_type_to_loader(js_module.media_type),
      ),
      deno_graph::Module::Json(json_module) => (
        json_module.source.as_bytes().to_vec(),
        esbuild_rs::BuiltinLoader::Json,
      ),
      deno_graph::Module::Wasm(wasm_module) => todo!(),
      deno_graph::Module::Npm(module) => {
        let package_folder = npm_resolver
          .as_managed()
          .unwrap() // byonm won't create a Module::Npm
          .resolve_pkg_folder_from_deno_module(module.nv_reference.nv())?;
        let path = node_resolver.resolve_package_subpath_from_deno_module(
          &package_folder,
          module.nv_reference.sub_path(),
          None,
          ResolutionMode::Import,
          NodeResolutionKind::Execution,
        )?;
        let url = path.clone().into_url()?;
        let path = path.into_path()?;
        let (media_type, _charset) =
          deno_media_type::resolve_media_type_and_charset_from_content_type(
            &url, None,
          );
        let contents = std::fs::read(path)?;
        (contents, media_type_to_loader(media_type))
      }
      deno_graph::Module::Node(built_in_node_module) => {
        return Ok(None);
      }
      deno_graph::Module::External(external_module) => {
        return Ok(None);
      }
    };

    Ok(Some((code, loader)))
  }
}

fn file_path_or_url(url: &Url) -> String {
  if url.scheme() == "file" {
    url.to_file_path().unwrap().to_string_lossy().to_string()
  } else {
    url.to_string()
  }
}
fn media_type_to_loader(
  media_type: deno_media_type::MediaType,
) -> esbuild_rs::BuiltinLoader {
  use deno_ast::MediaType::*;
  match media_type {
    JavaScript | Cjs | Mjs | Mts => esbuild_rs::BuiltinLoader::Js,
    TypeScript | Cts | Dts | Dmts | Dcts => esbuild_rs::BuiltinLoader::Ts,
    Jsx | Tsx => esbuild_rs::BuiltinLoader::Jsx,
    Css => esbuild_rs::BuiltinLoader::Css,
    Json => esbuild_rs::BuiltinLoader::Json,
    SourceMap => esbuild_rs::BuiltinLoader::Text,
    Html => esbuild_rs::BuiltinLoader::Text,
    Sql => esbuild_rs::BuiltinLoader::Text,
    Wasm => esbuild_rs::BuiltinLoader::Binary,
    Unknown => esbuild_rs::BuiltinLoader::Binary,
    // _ => esbuild_rs::BuiltinLoader::External,
  }
}
