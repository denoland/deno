// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::sync::Arc;

use deno_ast::EmitOptions;
use deno_ast::TranspileOptions;
use deno_error::JsErrorBox;
use deno_graph::BuildOptions;
use deno_graph::GraphKind;
use deno_graph::ModuleGraph;
use deno_graph::ast::CapturingModuleAnalyzer;
use deno_graph::source::CacheSetting;
use deno_graph::source::ResolveError;
use import_map::ImportMap;
use reqwest::StatusCode;
use url::Url;

#[tokio::main(flavor = "current_thread")]
async fn main() {
  let args = std::env::args().collect::<Vec<_>>();
  let url = args.get(1).unwrap();
  let url = Url::parse(url).unwrap();
  let out = args.get(2).unwrap();
  let maybe_import_map = args.get(3).map(|url| Url::parse(url).unwrap());

  let loader = Loader;
  let (maybe_import_map, maybe_import_map_data) =
    if let Some(import_map_url) = maybe_import_map {
      let resp = deno_graph::source::Loader::load(
        &loader,
        &import_map_url,
        deno_graph::source::LoadOptions {
          in_dynamic_branch: false,
          was_dynamic_root: false,
          cache_setting: CacheSetting::Use,
          maybe_checksum: None,
        },
      )
      .await
      .unwrap()
      .unwrap();
      match resp {
        deno_graph::source::LoadResponse::Module {
          specifier, content, ..
        } => {
          let content = String::from_utf8(content.to_vec()).unwrap();
          let import_map =
            import_map::parse_from_json(specifier.clone(), &content).unwrap();
          (Some(import_map.import_map), Some((specifier, content)))
        }
        _ => unimplemented!(),
      }
    } else {
      (None, None)
    };

  let analyzer = CapturingModuleAnalyzer::default();

  let mut graph = ModuleGraph::new(GraphKind::CodeOnly);
  graph
    .build(
      vec![url],
      Vec::new(),
      &loader,
      BuildOptions {
        resolver: Some(&Resolver(maybe_import_map)),
        module_analyzer: &analyzer,
        ..Default::default()
      },
    )
    .await;

  graph.valid().unwrap();

  let mut eszip = eszip::EszipV2::from_graph(eszip::FromGraphOptions {
    graph,
    parser: analyzer.as_capturing_parser(),
    module_kind_resolver: Default::default(),
    transpile_options: TranspileOptions::default(),
    emit_options: EmitOptions::default(),
    relative_file_base: None,
    npm_packages: None,
    npm_snapshot: Default::default(),
  })
  .unwrap();
  if let Some((import_map_specifier, import_map_content)) =
    maybe_import_map_data
  {
    eszip.add_import_map(
      eszip::ModuleKind::Json,
      import_map_specifier.to_string(),
      Arc::from(import_map_content.into_bytes()),
    )
  }
  for specifier in eszip.specifiers() {
    println!("source: {specifier}")
  }

  let bytes = eszip.into_bytes();

  std::fs::write(out, bytes).unwrap();
}

#[derive(Debug)]
struct Resolver(Option<ImportMap>);

impl deno_graph::source::Resolver for Resolver {
  fn resolve(
    &self,
    specifier: &str,
    referrer_range: &deno_graph::Range,
    _kind: deno_graph::source::ResolutionKind,
  ) -> Result<deno_graph::ModuleSpecifier, ResolveError> {
    if let Some(import_map) = &self.0 {
      import_map
        .resolve(specifier, &referrer_range.specifier)
        .map_err(ResolveError::from_err)
    } else {
      Ok(deno_graph::resolve_import(
        specifier,
        &referrer_range.specifier,
      )?)
    }
  }
}

struct Loader;

impl deno_graph::source::Loader for Loader {
  fn load(
    &self,
    specifier: &deno_graph::ModuleSpecifier,
    _options: deno_graph::source::LoadOptions,
  ) -> deno_graph::source::LoadFuture {
    let specifier = specifier.clone();

    Box::pin(async move {
      match specifier.scheme() {
        "data" => {
          deno_graph::source::load_data_url(&specifier).map_err(|err| {
            deno_graph::source::LoadError::Other(Arc::new(
              JsErrorBox::from_err(err),
            ))
          })
        }
        "file" => {
          let path = std::fs::canonicalize(specifier.to_file_path().unwrap())
            .map_err(|err| {
            deno_graph::source::LoadError::Other(Arc::new(
              JsErrorBox::from_err(err),
            ))
          })?;
          let content = std::fs::read(&path).map_err(|err| {
            deno_graph::source::LoadError::Other(Arc::new(
              JsErrorBox::from_err(err),
            ))
          })?;
          Ok(Some(deno_graph::source::LoadResponse::Module {
            specifier: Url::from_file_path(&path).unwrap(),
            maybe_headers: None,
            mtime: None,
            content: Arc::from(content),
          }))
        }
        "http" | "https" => {
          let resp = reqwest::get(specifier.as_str()).await.map_err(|err| {
            deno_graph::source::LoadError::Other(Arc::new(JsErrorBox::generic(
              err.to_string(),
            )))
          })?;
          if resp.status() == StatusCode::NOT_FOUND {
            Ok(None)
          } else {
            let resp = resp.error_for_status().map_err(|err| {
              deno_graph::source::LoadError::Other(Arc::new(
                JsErrorBox::generic(err.to_string()),
              ))
            })?;
            let mut headers = HashMap::new();
            for key in resp.headers().keys() {
              let key_str = key.to_string();
              let values = resp.headers().get_all(key);
              let values_str = values
                .iter()
                .filter_map(|e| e.to_str().ok())
                .collect::<Vec<&str>>()
                .join(",");
              headers.insert(key_str, values_str);
            }
            let url = resp.url().clone();
            let content = resp.bytes().await.map_err(|err| {
              deno_graph::source::LoadError::Other(Arc::new(
                JsErrorBox::generic(err.to_string()),
              ))
            })?;
            Ok(Some(deno_graph::source::LoadResponse::Module {
              specifier: url,
              mtime: None,
              maybe_headers: Some(headers),
              content: Arc::from(content.as_ref()),
            }))
          }
        }
        _ => {
          let err: Arc<dyn deno_error::JsErrorClass> =
            Arc::new(JsErrorBox::generic(format!(
              "unsupported scheme: {}",
              specifier.scheme()
            )));
          Err(deno_graph::source::LoadError::Other(err))
        }
      }
    })
  }
}
