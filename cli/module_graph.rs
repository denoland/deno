// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![allow(unused)]

use crate::file_fetcher::SourceFileFetcher;
use crate::swc_util::analyze_dependencies;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
struct ModuleGraph(HashMap<String, ModuleGraphFile>);

#[derive(Debug, Serialize)]
struct ModuleGraphFile {
  pub specifier: String,
  pub deps: Vec<String>,

  // pub imports: Vec<ImportDescriptor>,
  // pub referenced_files: Vec<ReferenceDescriptor>,
  // pub lib_directives: Vec<LibDirective>,
  // pub types_directives: Vec<TypeDirective>,
}

struct ModuleGraphLoader {
  file_fetcher: SourceFileFetcher,
  to_visit: Vec<ModuleSpecifier>,
  pub graph: ModuleGraph,
}

impl ModuleGraphLoader {
  pub fn new(file_fetcher: SourceFileFetcher) -> Self {
    Self {
      file_fetcher,
      to_visit: vec![],
      graph: ModuleGraph(HashMap::new()),
    }
  }

  pub async fn build_graph(
    mut self,
    specifier: &ModuleSpecifier,
  ) -> Result<HashMap<String, ModuleGraphFile>, ErrBox> {
    self.to_visit.push(specifier.to_owned());
    while let Some(spec) = self.to_visit.pop() {
      self.visit_module(&spec).await?;
      let file = self.graph.0.get(&spec.to_string()).unwrap();
      for dep in &file.deps {
        if let Some(_exists) = self.graph.0.get(dep) {
          continue;
        } else {
          self
            .to_visit
            .push(ModuleSpecifier::resolve_url_or_path(dep).unwrap());
        }
      }
    }
    Ok(self.graph.0)
  }

  async fn visit_module(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Result<(), ErrBox> {
    if self.graph.0.contains_key(&specifier.to_string()) {
      return Ok(());
    }

    let source_file =
      self.file_fetcher.fetch_source_file(specifier, None).await?;

    let raw_deps =
      analyze_dependencies(&String::from_utf8(source_file.source_code)?, true)?;

    // TODO(bartlomieju): apply import map, using State
    //    or should it be passed explicitly
    let mut deps = vec![];
    for raw_dep in raw_deps {
      let specifier =
        ModuleSpecifier::resolve_import(&raw_dep, &specifier.to_string())?;
      deps.push(specifier.to_string());
    }

    self.graph.0.insert(
      specifier.to_string(),
      ModuleGraphFile {
        specifier: specifier.to_string(),
        deps,
      },
    );
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::GlobalState;
  use std::path::PathBuf;

  // fn rel_module_specifier(relpath: &str) -> ModuleSpecifier {
  //   let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
  //     .join(relpath)
  //     .into_os_string();
  //   let ps = p.to_str().unwrap();
  //   ModuleSpecifier::resolve_url_or_path(ps).unwrap()
  // }

  #[tokio::test]
  async fn source_graph_fetch() {
    let http_server_guard = crate::test_util::http_server();

    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/019_media_types.ts",
    )
    .unwrap();
    let graph_loader =
      ModuleGraphLoader::new(global_state.file_fetcher.clone());
    let graph = graph_loader.build_graph(&module_specifier).await.unwrap();

    assert_eq!(
      serde_json::to_value(&graph).unwrap(),
      json!({
        "http://localhost:4545/cli/tests/subdir/mt_text_typescript.t1.ts": {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_text_typescript.t1.ts",
          "deps": []
        },
        "http://localhost:4545/cli/tests/019_media_types.ts": {
          "specifier": "http://localhost:4545/cli/tests/019_media_types.ts",
          "deps": [
            "http://localhost:4545/cli/tests/subdir/mt_text_typescript.t1.ts",
            "http://localhost:4545/cli/tests/subdir/mt_video_vdn.t2.ts",
            "http://localhost:4545/cli/tests/subdir/mt_video_mp2t.t3.ts",
            "http://localhost:4545/cli/tests/subdir/mt_application_x_typescript.t4.ts",
            "http://localhost:4545/cli/tests/subdir/mt_text_javascript.j1.js",
            "http://localhost:4545/cli/tests/subdir/mt_application_ecmascript.j2.js",
            "http://localhost:4545/cli/tests/subdir/mt_text_ecmascript.j3.js",
            "http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js"
          ]
        },
        "http://localhost:4545/cli/tests/subdir/mt_text_ecmascript.j3.js": {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_text_ecmascript.j3.js",
          "deps": []
        },
        "http://localhost:4545/cli/tests/subdir/mt_video_vdn.t2.ts": {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_video_vdn.t2.ts",
          "deps": []
        },
        "http://localhost:4545/cli/tests/subdir/mt_application_x_typescript.t4.ts": {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_application_x_typescript.t4.ts",
          "deps": []
        },
        "http://localhost:4545/cli/tests/subdir/mt_video_mp2t.t3.ts": {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_video_mp2t.t3.ts",
          "deps": []
        },
        "http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js": {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js",
          "deps": []
        },
        "http://localhost:4545/cli/tests/subdir/mt_application_ecmascript.j2.js": {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_application_ecmascript.j2.js",
          "deps": []
        },
        "http://localhost:4545/cli/tests/subdir/mt_text_javascript.j1.js": {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_text_javascript.j1.js",
          "deps": []
        }
      })
    );
    drop(http_server_guard);
  }

  #[tokio::test]
  async fn source_graph_fetch_circular() {
    let http_server_guard = crate::test_util::http_server();

    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/circular1.js",
    )
    .unwrap();

    let graph_loader =
      ModuleGraphLoader::new(global_state.file_fetcher.clone());
    let graph = graph_loader.build_graph(&module_specifier).await.unwrap();

    assert_eq!(
      serde_json::to_value(&graph).unwrap(),
      json!({
        "http://localhost:4545/cli/tests/circular2.js": {
          "specifier": "http://localhost:4545/cli/tests/circular2.js",
          "deps": [
            "http://localhost:4545/cli/tests/circular1.js"
          ]
        },
        "http://localhost:4545/cli/tests/circular1.js": {
          "specifier": "http://localhost:4545/cli/tests/circular1.js",
          "deps": [
            "http://localhost:4545/cli/tests/circular2.js"
          ]
        }
      })
    );
    drop(http_server_guard);
  }

  #[tokio::test]
  async fn source_graph_type_references() {
    let http_server_guard = crate::test_util::http_server();

    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/type_definitions.ts",
    )
    .unwrap();

    let graph_loader =
      ModuleGraphLoader::new(global_state.file_fetcher.clone());
    let graph = graph_loader.build_graph(&module_specifier).await.unwrap();

    assert_eq!(
      serde_json::to_value(&graph).unwrap(),
      json!({
        "http://localhost:4545/cli/tests/type_definitions.ts": {
          "imports": [
            {
              "specifier": "./type_definitions/foo.js",
              "resolvedUrl": "http://localhost:4545/cli/tests/type_definitions/foo.js"
              "typeDirective": "./type_definitions/foo.d.ts",
              "resolvedTypeDirective": "http://localhost:4545/cli/tests/type_definitions/foo.d.ts"
            },
            {
              "specifier": "./type_definitions/fizz.js",
              "resolvedUrl": "http://localhost:4545/cli/tests/type_definitions/fizz.js"
              "typeDirective": "./type_definitions/fizz.d.ts",
              "resolvedTypeDirective": "http://localhost:4545/cli/tests/type_definitions/fizz.d.ts"
            },
            {
              "specifier": "./type_definitions/qat.js",
              "resolvedUrl": "http://localhost:4545/cli/tests/type_definitions/qat.js"
            },
          ]
        },
        "http://localhost:4545/cli/tests/type_definitions/foo.js": {
          "imports": [],
        },
        "http://localhost:4545/cli/tests/type_definitions/foo.d.ts": {
          "imports": [],
        },
        "http://localhost:4545/cli/tests/type_definitions/fizz.js": {
          "imports": [],
        },
        "http://localhost:4545/cli/tests/type_definitions/fizz.d.ts": {
          "imports": [],
        },
        "http://localhost:4545/cli/tests/type_definitions/qat.js": {
          "imports": [],
        }
      })
    );
    drop(http_server_guard);
  }

  #[tokio::test]
  async fn source_graph_type_references2() {
    let http_server_guard = crate::test_util::http_server();

    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/type_directives_02.ts",
    )
    .unwrap();

    let graph_loader =
      ModuleGraphLoader::new(global_state.file_fetcher.clone());
    let graph = graph_loader.build_graph(&module_specifier).await.unwrap();

    assert_eq!(
      serde_json::to_value(&graph).unwrap(),
      json!({
        "http://localhost:4545/cli/tests/type_directives_02.ts": {
          "imports": [
            {
              "specifier": "./subdir/type_reference.js",
              "resolvedUrl": "http://localhost:4545/cli/tests/subdir/type_reference.js"
            }
          ]
        },
        "http://localhost:4545/cli/tests/subdir/type_reference.js": {
          "typeReferences": [
            {
              "specifier": "./type_reference.d.ts",
              "resolvedUrl": "http://localhost:4545/cli/tests/subdir/type_reference.d.ts"
            }
          ],
        }
      })
    );
    drop(http_server_guard);
  }

  #[tokio::test]
  async fn source_graph_type_references3() {
    let http_server_guard = crate::test_util::http_server();

    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/type_directives_01.ts",
    )
    .unwrap();

    let graph_loader =
      ModuleGraphLoader::new(global_state.file_fetcher.clone());
    let graph = graph_loader.build_graph(&module_specifier).await.unwrap();

    assert_eq!(
      serde_json::to_value(&graph).unwrap(),
      json!({
        "http://localhost:4545/cli/tests/type_directives_01.ts": {
          "imports": [
            {
              "specifier": "./xTypeScriptTypes.js",
              "resolvedUrl": "http://localhost:4545/cli/tests/xTypeScriptTypes.js"
            }
          ]
        },
        "http://localhost:4545/cli/tests/xTypeScriptTypes.js": {
          "typeHeaders": [
            {
              "specifier": "./xTypeScriptTypes.d.ts",
              "resolvedUrl": "http://localhost:4545/cli/tests/xTypeScriptTypes.d.ts"
            }
          ],
        }
      })
    );
    drop(http_server_guard);
  }
}
