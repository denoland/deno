use crate::graph_container::MainModuleGraphContainer;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::module_loader::ModuleLoadPreparer;
use crate::module_loader::PrepareModuleLoadOptions;
use crate::node::CliNodeResolver;
use crate::npm::CliNpmResolver;
use crate::resolver::CliDenoResolver;
use crate::resolver::CliNpmReqResolver;
// use crate::sys::CliSys;
use deno_config::deno_json::TsTypeLib;
use deno_core::op2;
use deno_core::serde_json::Value;
// use deno_core::AsyncRefCell;
// use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_error::JsError;
// use deno_error::JsErrorBox;
use deno_resolver::npm::managed::ResolvePkgFolderFromDenoModuleError;
// use deno_resolver::workspace::ResolutionKind;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_semver::npm::NpmPackageReqReference;
use node_resolver::errors::PackageSubpathResolveError;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
#[derive(Clone)]
struct BundleState {
  resolver: Arc<CliDenoResolver>,
  module_load_preparer: Arc<ModuleLoadPreparer>,
  module_graph_container: Arc<MainModuleGraphContainer>,
  permissions: PermissionsContainer,
  npm_req_resolver: Arc<CliNpmReqResolver>,
  npm_resolver: CliNpmResolver,
  node_resolver: Arc<CliNodeResolver>,
}
deno_core::extension!(
  deno_bundle_ext,
  ops = [op_bundle_resolve, op_bundle_load],
  esm_entry_point = "40_bundle.js",
  esm = [
    dir "../../js",
    "40_bundle.js"
  ],
  options = {
    resolver: Arc<CliDenoResolver>,
    module_load_preparer: Arc<ModuleLoadPreparer>,
    module_graph_container: Arc<MainModuleGraphContainer>,
    permissions: PermissionsContainer,
    npm_req_resolver: Arc<CliNpmReqResolver>,
    npm_resolver: CliNpmResolver,
    node_resolver: Arc<CliNodeResolver>,
  },
  state = |state, options| {
    state.put(BundleState {
      resolver: options.resolver,
      module_load_preparer: options.module_load_preparer,
      module_graph_container: options.module_graph_container,
      permissions: options.permissions,
      npm_req_resolver: options.npm_req_resolver,
      npm_resolver: options.npm_resolver,
      node_resolver: options.node_resolver,
    });
  }
);

/*
export interface OnResolveArgs {
  path: string;
  importer: string;
  namespace: string;
  resolveDir: string;
  kind: ImportKind;
  pluginData: any;
  with: Record<string, string>;
}
 */

/*
export type ImportKind =
 | "entry-point"
 // JS
 | "import-statement"
 | "require-call"
 | "dynamic-import"
 | "require-resolve"
 // CSS
 | "import-rule"
 | "composes-from"
 | "url-token"; */

#[derive(Debug, thiserror::Error, JsError)]
#[class(generic)]
enum BundleError {
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
}

#[op2]
#[string]
fn op_bundle_resolve(
  state: &OpState,
  #[string] path: String,
  #[string] importer: String,
  #[string] resolve_dir: String,
  #[string] kind: String,
  #[serde] with: Option<Value>,
) -> Result<Option<String>, BundleError> {
  // eprintln!(
  //   "op_bundle_resolve: {} {} {} {} {:?}",
  //   path, importer, resolve_dir, kind, with
  // );
  let mut resolve_dir = resolve_dir;
  let bundle_state = { state.borrow::<BundleState>().clone() };
  let resolver = bundle_state.resolver.clone();
  if !resolve_dir.ends_with(std::path::MAIN_SEPARATOR) {
    resolve_dir.push(std::path::MAIN_SEPARATOR);
  }
  let resolve_dir_path = Path::new(&resolve_dir);
  let mut referrer =
    (deno_core::resolve_url_or_path(&importer, resolve_dir_path))
      .unwrap_or_else(|_| {
        (deno_core::url::Url::from_directory_path(
          std::env::current_dir().unwrap(),
        ))
        .unwrap()
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
  let result = resolver.resolve(
    &path,
    &referrer,
    ResolutionMode::Import,
    node_resolver::NodeResolutionKind::Execution,
  );

  // eprintln!("op_bundle_resolve result: {:?}", result);
  // return error, but don't error out. defer until after all plugins have run. if still
  // nothing resolved it, then error out with this error.
  match result {
    Ok(result) => {
      if result.url.scheme() == "npm" {
        let req_ref = NpmPackageReqReference::from_specifier(&result.url)?;
        Ok(Some(
          bundle_state
            .npm_req_resolver
            .resolve_req_reference(
              &req_ref,
              &referrer,
              ResolutionMode::Import,
              NodeResolutionKind::Execution,
            )?
            .to_string_lossy()
            .to_string(),
        ))
      } else {
        Ok(Some(result.url.to_string()))
      }
    }
    Err(e) => Err(BundleError::Resolver(e)),
  }
}

fn media_type_to_loader(media_type: deno_media_type::MediaType) -> String {
  match media_type {
    deno_ast::MediaType::JavaScript => "js",
    deno_ast::MediaType::Jsx => "jsx",
    deno_ast::MediaType::Mjs => "mjs",
    deno_ast::MediaType::Cjs => "cjs",
    deno_ast::MediaType::TypeScript => "ts",
    deno_ast::MediaType::Mts => "mts",
    deno_ast::MediaType::Cts => "cts",
    deno_ast::MediaType::Dts => "dts",
    deno_ast::MediaType::Dmts => "dmts",
    deno_ast::MediaType::Dcts => "dcts",
    deno_ast::MediaType::Tsx => "tsx",
    deno_ast::MediaType::Css => "css",
    deno_ast::MediaType::Json => "json",
    deno_ast::MediaType::Html => "html",
    deno_ast::MediaType::Sql => "sql",
    deno_ast::MediaType::Wasm => "wasm",
    deno_ast::MediaType::SourceMap => "map",
    deno_ast::MediaType::Unknown => "external",
  }
  .to_string()
}

#[op2(async)]
#[serde]
async fn op_bundle_load(
  state: Rc<RefCell<OpState>>,
  #[string] specifier: String,
  #[string] resolve_dir: String,
) -> Result<Option<(String, String)>, BundleError> {
  // eprintln!("op_bundle_load: {}", specifier);
  let bundle_state = {
    let state = state.borrow();
    state.borrow::<BundleState>().clone()
  };
  let module_load_preparer = bundle_state.module_load_preparer.clone();
  let module_graph_container = bundle_state.module_graph_container.clone();
  let resolve_dir = Path::new(&resolve_dir);
  let specifier = deno_core::resolve_url_or_path(&specifier, resolve_dir)?;

  // let npm_req_resolver = bundle_state.npm_req_resolver.clone();
  let npm_resolver = bundle_state.npm_resolver.clone();
  let node_resolver = bundle_state.node_resolver.clone();
  {
    let mut graph_permit = module_graph_container.acquire_update_permit().await;
    let graph: &mut deno_graph::ModuleGraph = graph_permit.graph_mut();
    // eprintln!("about to prepare module load");
    let _prepared = module_load_preparer
      .prepare_module_load(
        graph,
        &[specifier.clone()],
        PrepareModuleLoadOptions {
          is_dynamic: false,
          lib: TsTypeLib::default(),
          permissions: bundle_state.permissions.clone(),
          ext_overwrite: None,
          allow_unknown_media_types: false,
        },
      )
      .await
      .inspect_err(|_e| {
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
      js_module.source.to_string(),
      media_type_to_loader(js_module.media_type),
    ),
    deno_graph::Module::Json(json_module) => {
      (json_module.source.to_string(), "json".to_string())
    }
    deno_graph::Module::Wasm(_wasm_module) => todo!(),
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
      let contents = std::fs::read_to_string(path)?;
      (contents, media_type_to_loader(media_type))
    }
    deno_graph::Module::Node(_built_in_node_module) => {
      return Ok(None);
    }
    deno_graph::Module::External(_external_module) => {
      return Ok(None);
    }
  };

  Ok(Some((code, loader)))
}
