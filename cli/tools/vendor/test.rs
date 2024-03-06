// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::FutureExt;
use deno_core::serde_json;
use deno_graph::source::LoadFuture;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_graph::DefaultModuleAnalyzer;
use deno_graph::GraphKind;
use deno_graph::ModuleGraph;
use import_map::ImportMap;

use crate::args::JsxImportSourceConfig;
use crate::cache::ParsedSourceCache;
use crate::resolver::CliGraphResolver;
use crate::resolver::CliGraphResolverOptions;

use super::build::VendorEnvironment;

// Utilities that help `deno vendor` get tested in memory.

type RemoteFileText = String;
type RemoteFileHeaders = Option<HashMap<String, String>>;
type RemoteFileResult = Result<(RemoteFileText, RemoteFileHeaders), String>;

#[derive(Clone, Default)]
pub struct TestLoader {
  files: HashMap<ModuleSpecifier, RemoteFileResult>,
  redirects: HashMap<ModuleSpecifier, ModuleSpecifier>,
}

impl TestLoader {
  pub fn add(
    &mut self,
    path_or_specifier: impl AsRef<str>,
    text: impl AsRef<str>,
  ) -> &mut Self {
    self.add_result(path_or_specifier, Ok((text.as_ref().to_string(), None)))
  }

  pub fn add_failure(
    &mut self,
    path_or_specifier: impl AsRef<str>,
    message: impl AsRef<str>,
  ) -> &mut Self {
    self.add_result(path_or_specifier, Err(message.as_ref().to_string()))
  }

  fn add_result(
    &mut self,
    path_or_specifier: impl AsRef<str>,
    result: RemoteFileResult,
  ) -> &mut Self {
    if path_or_specifier
      .as_ref()
      .to_lowercase()
      .starts_with("http")
    {
      self.files.insert(
        ModuleSpecifier::parse(path_or_specifier.as_ref()).unwrap(),
        result,
      );
    } else {
      let path = make_path(path_or_specifier.as_ref());
      let specifier = ModuleSpecifier::from_file_path(path).unwrap();
      self.files.insert(specifier, result);
    }
    self
  }

  pub fn add_with_headers(
    &mut self,
    specifier: impl AsRef<str>,
    text: impl AsRef<str>,
    headers: &[(&str, &str)],
  ) -> &mut Self {
    let headers = headers
      .iter()
      .map(|(key, value)| (key.to_string(), value.to_string()))
      .collect();
    self.files.insert(
      ModuleSpecifier::parse(specifier.as_ref()).unwrap(),
      Ok((text.as_ref().to_string(), Some(headers))),
    );
    self
  }

  pub fn add_redirect(
    &mut self,
    from: impl AsRef<str>,
    to: impl AsRef<str>,
  ) -> &mut Self {
    self.redirects.insert(
      ModuleSpecifier::parse(from.as_ref()).unwrap(),
      ModuleSpecifier::parse(to.as_ref()).unwrap(),
    );
    self
  }
}

impl Loader for TestLoader {
  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    _options: deno_graph::source::LoadOptions,
  ) -> LoadFuture {
    let specifier = self.redirects.get(specifier).unwrap_or(specifier);
    let result = self.files.get(specifier).map(|result| match result {
      Ok(result) => Ok(LoadResponse::Module {
        specifier: specifier.clone(),
        content: result.0.clone().into_bytes().into(),
        maybe_headers: result.1.clone(),
      }),
      Err(err) => Err(err),
    });
    let result = match result {
      Some(Ok(result)) => Ok(Some(result)),
      Some(Err(err)) => Err(anyhow!("{}", err)),
      None if specifier.scheme() == "data" => {
        deno_graph::source::load_data_url(specifier)
      }
      None => Ok(None),
    };
    Box::pin(futures::future::ready(result))
  }
}

#[derive(Default)]
struct TestVendorEnvironment {
  directories: RefCell<HashSet<PathBuf>>,
  files: RefCell<HashMap<PathBuf, String>>,
}

impl VendorEnvironment for TestVendorEnvironment {
  fn cwd(&self) -> Result<PathBuf, AnyError> {
    Ok(make_path("/"))
  }

  fn create_dir_all(&self, dir_path: &Path) -> Result<(), AnyError> {
    let mut directories = self.directories.borrow_mut();
    for path in dir_path.ancestors() {
      if !directories.insert(path.to_path_buf()) {
        break;
      }
    }
    Ok(())
  }

  fn write_file(&self, file_path: &Path, text: &[u8]) -> Result<(), AnyError> {
    let parent = file_path.parent().unwrap();
    if !self.directories.borrow().contains(parent) {
      bail!("Directory not found: {}", parent.display());
    }
    self.files.borrow_mut().insert(
      file_path.to_path_buf(),
      String::from_utf8(text.to_vec()).unwrap(),
    );
    Ok(())
  }

  fn path_exists(&self, path: &Path) -> bool {
    self.files.borrow().contains_key(&path.to_path_buf())
  }
}

pub struct VendorOutput {
  pub files: Vec<(String, String)>,
  pub import_map: Option<serde_json::Value>,
}

#[derive(Default)]
pub struct VendorTestBuilder {
  entry_points: Vec<ModuleSpecifier>,
  loader: TestLoader,
  original_import_map: Option<ImportMap>,
  environment: TestVendorEnvironment,
  jsx_import_source_config: Option<JsxImportSourceConfig>,
}

impl VendorTestBuilder {
  pub fn with_default_setup() -> Self {
    let mut builder = VendorTestBuilder::default();
    builder.add_entry_point("/mod.ts");
    builder
  }

  pub fn resolve_to_url(&self, path: &str) -> ModuleSpecifier {
    ModuleSpecifier::from_file_path(make_path(path)).unwrap()
  }

  pub fn new_import_map(&self, base_path: &str) -> ImportMap {
    let base = self.resolve_to_url(base_path);
    ImportMap::new(base)
  }

  pub fn set_original_import_map(
    &mut self,
    import_map: ImportMap,
  ) -> &mut Self {
    self.original_import_map = Some(import_map);
    self
  }

  pub fn add_entry_point(&mut self, entry_point: impl AsRef<str>) -> &mut Self {
    let entry_point = make_path(entry_point.as_ref());
    self
      .entry_points
      .push(ModuleSpecifier::from_file_path(entry_point).unwrap());
    self
  }

  pub fn set_jsx_import_source_config(
    &mut self,
    jsx_import_source_config: JsxImportSourceConfig,
  ) -> &mut Self {
    self.jsx_import_source_config = Some(jsx_import_source_config);
    self
  }

  pub async fn build(&mut self) -> Result<VendorOutput, AnyError> {
    let output_dir = make_path("/vendor");
    let entry_points = self.entry_points.clone();
    let loader = self.loader.clone();
    let parsed_source_cache = ParsedSourceCache::default();
    let resolver = Arc::new(build_resolver(
      self.jsx_import_source_config.clone(),
      self.original_import_map.clone(),
    ));
    super::build::build(super::build::BuildInput {
      entry_points,
      build_graph: {
        let resolver = resolver.clone();
        move |entry_points| {
          async move {
            let analyzer = DefaultModuleAnalyzer::default();
            Ok(
              build_test_graph(
                entry_points,
                loader,
                resolver.as_graph_resolver(),
                &analyzer,
              )
              .await,
            )
          }
          .boxed_local()
        }
      },
      parsed_source_cache: &parsed_source_cache,
      output_dir: &output_dir,
      maybe_original_import_map: self.original_import_map.as_ref(),
      maybe_lockfile: None,
      maybe_jsx_import_source: self.jsx_import_source_config.as_ref(),
      resolver: resolver.as_graph_resolver(),
      environment: &self.environment,
    })
    .await?;

    let mut files = self.environment.files.borrow_mut();
    let import_map = files.remove(&output_dir.join("import_map.json"));
    let mut files = files
      .iter()
      .map(|(path, text)| (path_to_string(path), text.to_string()))
      .collect::<Vec<_>>();

    files.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(VendorOutput {
      import_map: import_map.map(|text| serde_json::from_str(&text).unwrap()),
      files,
    })
  }

  pub fn with_loader(&mut self, action: impl Fn(&mut TestLoader)) -> &mut Self {
    action(&mut self.loader);
    self
  }
}

fn build_resolver(
  maybe_jsx_import_source_config: Option<JsxImportSourceConfig>,
  original_import_map: Option<ImportMap>,
) -> CliGraphResolver {
  CliGraphResolver::new(CliGraphResolverOptions {
    node_resolver: None,
    npm_resolver: None,
    sloppy_imports_resolver: None,
    package_json_deps_provider: Default::default(),
    maybe_jsx_import_source_config,
    maybe_import_map: original_import_map.map(Arc::new),
    maybe_vendor_dir: None,
    bare_node_builtins_enabled: false,
  })
}

async fn build_test_graph(
  roots: Vec<ModuleSpecifier>,
  mut loader: TestLoader,
  resolver: &dyn deno_graph::source::Resolver,
  analyzer: &dyn deno_graph::ModuleAnalyzer,
) -> ModuleGraph {
  let mut graph = ModuleGraph::new(GraphKind::All);
  graph
    .build(
      roots,
      &mut loader,
      deno_graph::BuildOptions {
        resolver: Some(resolver),
        module_analyzer: Some(analyzer),
        ..Default::default()
      },
    )
    .await;
  graph
}

fn make_path(text: &str) -> PathBuf {
  // This should work all in memory. We're waiting on
  // https://github.com/servo/rust-url/issues/730 to provide
  // a cross platform path here
  assert!(text.starts_with('/'));
  if cfg!(windows) {
    PathBuf::from(format!("C:{}", text.replace('/', "\\")))
  } else {
    PathBuf::from(text)
  }
}

fn path_to_string<P>(path: P) -> String
where
  P: AsRef<Path>,
{
  let path = path.as_ref();
  // inverse of the function above
  let path = path.to_string_lossy();
  if cfg!(windows) {
    path.replace("C:\\", "\\").replace('\\', "/")
  } else {
    path.to_string()
  }
}
