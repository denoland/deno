// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![allow(unused)]

use crate::file_fetcher::SourceFileFetcher;
use crate::import_map::ImportMap;
use crate::msg::MediaType;
use crate::swc_util::analyze_dependencies_and_references;
use crate::swc_util::TsReferenceKind;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use serde::Serialize;
use serde::Serializer;
use std::collections::HashMap;

fn serialize_module_specifier<S>(
  spec: &ModuleSpecifier,
  s: S,
) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  s.serialize_str(&spec.to_string())
}

fn serialize_option_module_specifier<S>(
  maybe_spec: &Option<ModuleSpecifier>,
  s: S,
) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  if let Some(spec) = maybe_spec {
    serialize_module_specifier(spec, s)
  } else {
    s.serialize_none()
  }
}

#[derive(Debug, Serialize)]
pub struct ModuleGraph(HashMap<String, ModuleGraphFile>);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDescriptor {
  specifier: String,
  #[serde(serialize_with = "serialize_module_specifier")]
  resolved_specifier: ModuleSpecifier,
  // These two fields are for support of @deno-types directive
  // directly prepending import statement
  type_directive: Option<String>,
  #[serde(serialize_with = "serialize_option_module_specifier")]
  resolved_type_directive: Option<ModuleSpecifier>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceDescriptor {
  specifier: String,
  #[serde(serialize_with = "serialize_module_specifier")]
  resolved_specifier: ModuleSpecifier,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleGraphFile {
  pub specifier: String,
  pub imports: Vec<ImportDescriptor>,
  pub referenced_files: Vec<ReferenceDescriptor>,
  pub lib_directives: Vec<ReferenceDescriptor>,
  pub types_directives: Vec<ReferenceDescriptor>,
  pub type_headers: Vec<ReferenceDescriptor>,
  pub media_type: MediaType,
  pub source_code: String,
}

pub struct ModuleGraphLoader {
  file_fetcher: SourceFileFetcher,
  maybe_import_map: Option<ImportMap>,
  to_visit: Vec<ModuleSpecifier>,
  pub graph: ModuleGraph,
}

impl ModuleGraphLoader {
  pub fn new(
    file_fetcher: SourceFileFetcher,
    maybe_import_map: Option<ImportMap>,
  ) -> Self {
    Self {
      file_fetcher,
      maybe_import_map,
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
    }
    Ok(self.graph.0)
  }

  async fn visit_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<(), ErrBox> {
    if self.graph.0.contains_key(&module_specifier.to_string()) {
      return Ok(());
    }

    let source_file = self
      .file_fetcher
      .fetch_source_file(module_specifier, None)
      .await?;

    let mut imports = vec![];
    let mut referenced_files = vec![];
    let mut lib_directives = vec![];
    let mut types_directives = vec![];
    let mut type_headers = vec![];

    let source_code = String::from_utf8(source_file.source_code)?;

    if source_file.media_type == MediaType::JavaScript
      || source_file.media_type == MediaType::TypeScript
    {
      if let Some(types_specifier) = source_file.types_header {
        let type_header = ReferenceDescriptor {
          specifier: types_specifier.to_string(),
          resolved_specifier: ModuleSpecifier::resolve_import(
            &types_specifier,
            &module_specifier.to_string(),
          )?,
        };
        type_headers.push(type_header);
      }

      let (import_descs, ref_descs) =
        analyze_dependencies_and_references(&source_code, true)?;

      for import_desc in import_descs {
        let maybe_resolved =
          if let Some(import_map) = self.maybe_import_map.as_ref() {
            import_map
              .resolve(&import_desc.specifier, &module_specifier.to_string())?
          } else {
            None
          };

        let resolved_specifier = if let Some(resolved) = maybe_resolved {
          resolved
        } else {
          ModuleSpecifier::resolve_import(
            &import_desc.specifier,
            &module_specifier.to_string(),
          )?
        };

        let resolved_type_directive =
          if let Some(types_specifier) = import_desc.deno_types.as_ref() {
            Some(ModuleSpecifier::resolve_import(
              &types_specifier,
              &module_specifier.to_string(),
            )?)
          } else {
            None
          };

        let import_descriptor = ImportDescriptor {
          specifier: import_desc.specifier.to_string(),
          resolved_specifier,
          type_directive: import_desc.deno_types,
          resolved_type_directive,
        };

        if self
          .graph
          .0
          .get(&import_descriptor.resolved_specifier.to_string())
          .is_none()
        {
          self
            .to_visit
            .push(import_descriptor.resolved_specifier.clone());
        }

        if let Some(type_dir_url) =
          import_descriptor.resolved_type_directive.as_ref()
        {
          if self.graph.0.get(&type_dir_url.to_string()).is_none() {
            self.to_visit.push(type_dir_url.clone());
          }
        }

        imports.push(import_descriptor);
      }

      for ref_desc in ref_descs {
        let resolved_specifier = ModuleSpecifier::resolve_import(
          &ref_desc.specifier,
          &module_specifier.to_string(),
        )?;
        let reference_descriptor = ReferenceDescriptor {
          specifier: ref_desc.specifier.to_string(),
          resolved_specifier,
        };

        if self
          .graph
          .0
          .get(&reference_descriptor.resolved_specifier.to_string())
          .is_none()
        {
          self
            .to_visit
            .push(reference_descriptor.resolved_specifier.clone());
        }

        match ref_desc.kind {
          TsReferenceKind::Lib => {
            lib_directives.push(reference_descriptor);
          }
          TsReferenceKind::Types => {
            types_directives.push(reference_descriptor);
          }
          TsReferenceKind::Path => {
            referenced_files.push(reference_descriptor);
          }
        }
      }
    }

    self.graph.0.insert(
      module_specifier.to_string(),
      ModuleGraphFile {
        specifier: module_specifier.to_string(),
        media_type: source_file.media_type,
        source_code,
        imports,
        referenced_files,
        lib_directives,
        types_directives,
        type_headers,
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

  #[ignore]
  #[tokio::test]
  async fn source_graph_fetch() {
    let http_server_guard = crate::test_util::http_server();

    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/019_media_types.ts",
    )
    .unwrap();
    let graph_loader =
      ModuleGraphLoader::new(global_state.file_fetcher.clone(), None);
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

  #[ignore]
  #[tokio::test]
  async fn source_graph_fetch_circular() {
    let http_server_guard = crate::test_util::http_server();

    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/circular1.js",
    )
    .unwrap();

    let graph_loader =
      ModuleGraphLoader::new(global_state.file_fetcher.clone(), None);
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

  #[ignore]
  #[tokio::test]
  async fn source_graph_type_references() {
    let http_server_guard = crate::test_util::http_server();

    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/type_definitions.ts",
    )
    .unwrap();

    let graph_loader =
      ModuleGraphLoader::new(global_state.file_fetcher.clone(), None);
    let graph = graph_loader.build_graph(&module_specifier).await.unwrap();

    eprintln!("json {:#?}", serde_json::to_value(&graph).unwrap());

    assert_eq!(
      serde_json::to_value(&graph).unwrap(),
      json!({
        "http://localhost:4545/cli/tests/type_definitions.ts": {
          "specifier": "http://localhost:4545/cli/tests/type_definitions.ts",
          "imports": [
            {
              "specifier": "./type_definitions/foo.js",
              "resolvedSpecifier": "http://localhost:4545/cli/tests/type_definitions/foo.js",
              "typeDirective": "./type_definitions/foo.d.ts",
              "resolvedTypeDirective": "http://localhost:4545/cli/tests/type_definitions/foo.d.ts"
            },
            {
              "specifier": "./type_definitions/fizz.js",
              "resolvedSpecifier": "http://localhost:4545/cli/tests/type_definitions/fizz.js",
              "typeDirective": "./type_definitions/fizz.d.ts",
              "resolvedTypeDirective": "http://localhost:4545/cli/tests/type_definitions/fizz.d.ts"
            },
            {
              "specifier": "./type_definitions/qat.ts",
              "resolvedSpecifier": "http://localhost:4545/cli/tests/type_definitions/qat.ts",
              "typeDirective": null,
              "resolvedTypeDirective": null,
            },
          ],
          "typesDirectives": [],
          "referencedFiles": [],
          "libDirectives": [],
          "typeHeaders": [],
        },
        "http://localhost:4545/cli/tests/type_definitions/foo.js": {
          "specifier": "http://localhost:4545/cli/tests/type_definitions/foo.js",
          "imports": [],
          "referencedFiles": [],
          "libDirectives": [],
          "typesDirectives": [],
          "typeHeaders": [],
        },
        "http://localhost:4545/cli/tests/type_definitions/foo.d.ts": {
          "specifier": "http://localhost:4545/cli/tests/type_definitions/foo.d.ts",
          "imports": [],
          "referencedFiles": [],
          "libDirectives": [],
          "typesDirectives": [],
          "typeHeaders": [],
        },
        "http://localhost:4545/cli/tests/type_definitions/fizz.js": {
          "specifier": "http://localhost:4545/cli/tests/type_definitions/fizz.js",
          "imports": [],
          "referencedFiles": [],
          "libDirectives": [],
          "typesDirectives": [],
          "typeHeaders": [],
        },
        "http://localhost:4545/cli/tests/type_definitions/fizz.d.ts": {
          "specifier": "http://localhost:4545/cli/tests/type_definitions/fizz.d.ts",
          "imports": [],
          "referencedFiles": [],
          "libDirectives": [],
          "typesDirectives": [],
          "typeHeaders": [],
        },
        "http://localhost:4545/cli/tests/type_definitions/qat.ts": {
          "specifier": "http://localhost:4545/cli/tests/type_definitions/qat.ts",
          "imports": [],
          "referencedFiles": [],
          "libDirectives": [],
          "typesDirectives": [],
          "typeHeaders": [],
        }
      })
    );
    drop(http_server_guard);
  }

  #[ignore]
  #[tokio::test]
  async fn source_graph_type_references2() {
    let http_server_guard = crate::test_util::http_server();

    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/type_directives_02.ts",
    )
    .unwrap();

    let graph_loader =
      ModuleGraphLoader::new(global_state.file_fetcher.clone(), None);
    let graph = graph_loader.build_graph(&module_specifier).await.unwrap();

    eprintln!("{:#?}", serde_json::to_value(&graph).unwrap());

    assert_eq!(
      serde_json::to_value(&graph).unwrap(),
      json!({
        "http://localhost:4545/cli/tests/type_directives_02.ts": {
          "specifier": "http://localhost:4545/cli/tests/type_directives_02.ts",
          "imports": [
            {
              "specifier": "./subdir/type_reference.js",
              "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/type_reference.js",
              "typeDirective": null,
              "resolvedTypeDirective": null,
            }
          ],
          "typesDirectives": [],
          "referencedFiles": [],
          "libDirectives": [],
          "typeHeaders": [],
        },
        "http://localhost:4545/cli/tests/subdir/type_reference.d.ts": {
          "specifier": "http://localhost:4545/cli/tests/subdir/type_reference.d.ts",
          "imports": [],
          "referencedFiles": [],
          "libDirectives": [],
          "typesDirectives": [],
          "typeHeaders": [],
        },
        "http://localhost:4545/cli/tests/subdir/type_reference.js": {
          "specifier": "http://localhost:4545/cli/tests/subdir/type_reference.js",
          "imports": [],
          "referencedFiles": [],
          "libDirectives": [],
          "typesDirectives": [
            {
              "specifier": "./type_reference.d.ts",
              "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/type_reference.d.ts",
            }
          ],
          "typeHeaders": [],
        }
      })
    );
    drop(http_server_guard);
  }

  #[ignore]
  #[tokio::test]
  async fn source_graph_type_references3() {
    let http_server_guard = crate::test_util::http_server();

    let global_state = GlobalState::new(Default::default()).unwrap();
    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/type_directives_01.ts",
    )
    .unwrap();

    let graph_loader =
      ModuleGraphLoader::new(global_state.file_fetcher.clone(), None);
    let graph = graph_loader.build_graph(&module_specifier).await.unwrap();

    assert_eq!(
      serde_json::to_value(&graph).unwrap(),
      json!({
        "http://localhost:4545/cli/tests/type_directives_01.ts": {
          "specifier": "http://localhost:4545/cli/tests/type_directives_01.ts",
          "imports": [
            {
              "specifier": "http://127.0.0.1:4545/xTypeScriptTypes.js",
              "resolvedSpecifier": "http://127.0.0.1:4545/xTypeScriptTypes.js",
              "typeDirective": null,
              "resolvedTypeDirective": null,
            }
          ],
          "referencedFiles": [],
          "libDirectives": [],
          "typesDirectives": [],
          "typeHeaders": [],
        },
        "http://127.0.0.1:4545/xTypeScriptTypes.js": {
          "specifier": "http://127.0.0.1:4545/xTypeScriptTypes.js",
          "typeHeaders": [
            {
              "specifier": "./xTypeScriptTypes.d.ts",
              "resolvedSpecifier": "http://127.0.0.1:4545/xTypeScriptTypes.d.ts"
            }
          ],
          "imports": [],
          "referencedFiles": [],
          "libDirectives": [],
          "typesDirectives": [],
        }
      })
    );
    drop(http_server_guard);
  }
}
