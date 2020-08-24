// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::checksum;
use crate::file_fetcher::map_file_extension;
use crate::file_fetcher::SourceFile;
use crate::file_fetcher::SourceFileFetcher;
use crate::import_map::ImportMap;
use crate::msg::MediaType;
use crate::op_error::OpError;
use crate::permissions::Permissions;
use crate::swc_util::Location;
use crate::tsc::pre_process_file;
use crate::tsc::ImportDesc;
use crate::tsc::TsReferenceDesc;
use crate::tsc::TsReferenceKind;
use crate::tsc::AVAILABLE_LIBS;
use crate::version;
use deno_core::ErrBox;
use deno_core::ModuleSpecifier;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use futures::Future;
use futures::FutureExt;
use serde::Serialize;
use serde::Serializer;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::pin::Pin;

// TODO(bartlomieju): it'd be great if this function returned
// more structured data and possibly format the same as TS diagnostics.
/// Decorate error with location of import that caused the error.
fn err_with_location(e: ErrBox, maybe_location: Option<&Location>) -> ErrBox {
  if let Some(location) = maybe_location {
    let location_str = format!(
      "\nImported from \"{}:{}\"",
      location.filename, location.line
    );
    let err_str = e.to_string();
    OpError::other(format!("{}{}", err_str, location_str)).into()
  } else {
    e
  }
}

/// Disallow http:// imports from modules loaded over https://
fn validate_no_downgrade(
  module_specifier: &ModuleSpecifier,
  maybe_referrer: Option<&ModuleSpecifier>,
  maybe_location: Option<&Location>,
) -> Result<(), ErrBox> {
  if let Some(referrer) = maybe_referrer.as_ref() {
    if let "https" = referrer.as_url().scheme() {
      if let "http" = module_specifier.as_url().scheme() {
        let e = OpError::permission_denied(
          "Modules loaded over https:// are not allowed to import modules over http://".to_string()
        );
        return Err(err_with_location(e.into(), maybe_location));
      };
    };
  };

  Ok(())
}

/// Verify that remote file doesn't try to statically import local file.
fn validate_no_file_from_remote(
  module_specifier: &ModuleSpecifier,
  maybe_referrer: Option<&ModuleSpecifier>,
  maybe_location: Option<&Location>,
) -> Result<(), ErrBox> {
  if let Some(referrer) = maybe_referrer.as_ref() {
    let referrer_url = referrer.as_url();
    match referrer_url.scheme() {
      "http" | "https" => {
        let specifier_url = module_specifier.as_url();
        match specifier_url.scheme() {
          "http" | "https" => {}
          _ => {
            let e = OpError::permission_denied(
              "Remote modules are not allowed to statically import local modules. Use dynamic import instead.".to_string()
            );
            return Err(err_with_location(e.into(), maybe_location));
          }
        }
      }
      _ => {}
    }
  }

  Ok(())
}

// TODO(bartlomieju): handle imports/references in ambient contexts/TS modules
// https://github.com/denoland/deno/issues/6133
fn resolve_imports_and_references(
  referrer: ModuleSpecifier,
  maybe_import_map: Option<&ImportMap>,
  import_descs: Vec<ImportDesc>,
  ref_descs: Vec<TsReferenceDesc>,
) -> Result<(Vec<ImportDescriptor>, Vec<ReferenceDescriptor>), ErrBox> {
  let mut imports = vec![];
  let mut references = vec![];

  for import_desc in import_descs {
    let maybe_resolved = if let Some(import_map) = maybe_import_map.as_ref() {
      import_map.resolve(&import_desc.specifier, &referrer.to_string())?
    } else {
      None
    };

    let resolved_specifier = if let Some(resolved) = maybe_resolved {
      resolved
    } else {
      ModuleSpecifier::resolve_import(
        &import_desc.specifier,
        &referrer.to_string(),
      )?
    };

    let resolved_type_directive =
      if let Some(types_specifier) = import_desc.deno_types.as_ref() {
        Some(ModuleSpecifier::resolve_import(
          &types_specifier,
          &referrer.to_string(),
        )?)
      } else {
        None
      };

    let import_descriptor = ImportDescriptor {
      specifier: import_desc.specifier.to_string(),
      resolved_specifier,
      type_directive: import_desc.deno_types.clone(),
      resolved_type_directive,
      location: import_desc.location,
    };

    imports.push(import_descriptor);
  }

  for ref_desc in ref_descs {
    if AVAILABLE_LIBS.contains(&ref_desc.specifier.as_str()) {
      continue;
    }

    let resolved_specifier = ModuleSpecifier::resolve_import(
      &ref_desc.specifier,
      &referrer.to_string(),
    )?;

    let reference_descriptor = ReferenceDescriptor {
      specifier: ref_desc.specifier.to_string(),
      resolved_specifier,
      kind: ref_desc.kind,
      location: ref_desc.location,
    };

    references.push(reference_descriptor);
  }

  Ok((imports, references))
}

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

const SUPPORTED_MEDIA_TYPES: [MediaType; 4] = [
  MediaType::JavaScript,
  MediaType::TypeScript,
  MediaType::JSX,
  MediaType::TSX,
];

pub type ModuleGraph = HashMap<String, ModuleGraphFile>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDescriptor {
  pub specifier: String,
  #[serde(serialize_with = "serialize_module_specifier")]
  pub resolved_specifier: ModuleSpecifier,
  // These two fields are for support of @deno-types directive
  // directly prepending import statement
  pub type_directive: Option<String>,
  #[serde(serialize_with = "serialize_option_module_specifier")]
  pub resolved_type_directive: Option<ModuleSpecifier>,
  #[serde(skip)]
  pub location: Location,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceDescriptor {
  pub specifier: String,
  #[serde(serialize_with = "serialize_module_specifier")]
  pub resolved_specifier: ModuleSpecifier,
  #[serde(skip)]
  pub kind: TsReferenceKind,
  #[serde(skip)]
  pub location: Location,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleGraphFile {
  pub specifier: String,
  pub url: String,
  pub redirect: Option<String>,
  pub filename: String,
  pub version_hash: String,
  pub imports: Vec<ImportDescriptor>,
  pub referenced_files: Vec<ReferenceDescriptor>,
  pub lib_directives: Vec<ReferenceDescriptor>,
  pub types_directives: Vec<ReferenceDescriptor>,
  pub type_headers: Vec<ReferenceDescriptor>,
  pub media_type: MediaType,
  pub source_code: String,
}

type SourceFileFuture =
  Pin<Box<dyn Future<Output = Result<(ModuleSpecifier, SourceFile), ErrBox>>>>;

pub struct ModuleGraphLoader {
  permissions: Permissions,
  file_fetcher: SourceFileFetcher,
  maybe_import_map: Option<ImportMap>,
  pending_downloads: FuturesUnordered<SourceFileFuture>,
  has_downloaded: HashSet<ModuleSpecifier>,
  graph: ModuleGraph,
  is_dyn_import: bool,
  analyze_dynamic_imports: bool,
}

impl ModuleGraphLoader {
  pub fn new(
    file_fetcher: SourceFileFetcher,
    maybe_import_map: Option<ImportMap>,
    permissions: Permissions,
    is_dyn_import: bool,
    analyze_dynamic_imports: bool,
  ) -> Self {
    Self {
      file_fetcher,
      permissions,
      maybe_import_map,
      pending_downloads: FuturesUnordered::new(),
      has_downloaded: HashSet::new(),
      graph: ModuleGraph::new(),
      is_dyn_import,
      analyze_dynamic_imports,
    }
  }

  /// This method is used to add specified module and all of its
  /// dependencies to the graph.
  ///
  /// It resolves when all dependent modules have been fetched and analyzed.
  ///
  /// This method can be called multiple times.
  pub async fn add_to_graph(
    &mut self,
    specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
  ) -> Result<(), ErrBox> {
    self.download_module(specifier.clone(), maybe_referrer, None)?;

    loop {
      let (specifier, source_file) =
        self.pending_downloads.next().await.unwrap()?;
      self.visit_module(&specifier, source_file)?;
      if self.pending_downloads.is_empty() {
        break;
      }
    }

    Ok(())
  }

  /// This method is used to create a graph from in-memory files stored in
  /// a hash map. Useful for creating module graph for code received from
  /// the runtime.
  pub fn build_local_graph(
    &mut self,
    _root_name: &str,
    source_map: &HashMap<String, String>,
  ) -> Result<(), ErrBox> {
    for (spec, source_code) in source_map.iter() {
      self.visit_memory_module(spec.to_string(), source_code.to_string())?;
    }

    Ok(())
  }

  /// Consumes the loader and returns created graph.
  pub fn get_graph(self) -> ModuleGraph {
    self.graph
  }

  fn visit_memory_module(
    &mut self,
    specifier: String,
    source_code: String,
  ) -> Result<(), ErrBox> {
    let mut referenced_files = vec![];
    let mut lib_directives = vec![];
    let mut types_directives = vec![];

    // FIXME(bartlomieju):
    // The resolveModules op only handles fully qualified URLs for referrer.
    // However we will have cases where referrer is "/foo.ts". We add this dummy
    // prefix "memory://" in order to use resolution logic.
    let module_specifier =
      if let Ok(spec) = ModuleSpecifier::resolve_url(&specifier) {
        spec
      } else {
        ModuleSpecifier::resolve_url(&format!("memory://{}", specifier))?
      };

    let (raw_imports, raw_references) = pre_process_file(
      &module_specifier.to_string(),
      map_file_extension(&PathBuf::from(&specifier)),
      &source_code,
      self.analyze_dynamic_imports,
    )?;
    let (imports, references) = resolve_imports_and_references(
      module_specifier.clone(),
      self.maybe_import_map.as_ref(),
      raw_imports,
      raw_references,
    )?;

    for ref_descriptor in references {
      match ref_descriptor.kind {
        TsReferenceKind::Lib => {
          lib_directives.push(ref_descriptor);
        }
        TsReferenceKind::Types => {
          types_directives.push(ref_descriptor);
        }
        TsReferenceKind::Path => {
          referenced_files.push(ref_descriptor);
        }
      }
    }

    self.graph.insert(
      module_specifier.to_string(),
      ModuleGraphFile {
        specifier: specifier.to_string(),
        url: specifier.to_string(),
        redirect: None,
        version_hash: "".to_string(),
        media_type: map_file_extension(&PathBuf::from(specifier.clone())),
        filename: specifier,
        source_code,
        imports,
        referenced_files,
        lib_directives,
        types_directives,
        type_headers: vec![],
      },
    );
    Ok(())
  }

  // TODO(bartlomieju): decorate errors with import location in the source code
  // https://github.com/denoland/deno/issues/5080
  fn download_module(
    &mut self,
    module_specifier: ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
    maybe_location: Option<Location>,
  ) -> Result<(), ErrBox> {
    if self.has_downloaded.contains(&module_specifier) {
      return Ok(());
    }

    validate_no_downgrade(
      &module_specifier,
      maybe_referrer.as_ref(),
      maybe_location.as_ref(),
    )?;

    if !self.is_dyn_import {
      validate_no_file_from_remote(
        &module_specifier,
        maybe_referrer.as_ref(),
        maybe_location.as_ref(),
      )?;
    }

    self.has_downloaded.insert(module_specifier.clone());
    let spec = module_specifier;
    let file_fetcher = self.file_fetcher.clone();
    let perms = self.permissions.clone();

    let load_future = async move {
      let spec_ = spec.clone();
      let source_file = file_fetcher
        .fetch_source_file(&spec_, maybe_referrer, perms)
        .await
        .map_err(|e| err_with_location(e, maybe_location.as_ref()))?;

      Ok((spec_.clone(), source_file))
    }
    .boxed_local();

    self.pending_downloads.push(load_future);
    Ok(())
  }

  fn visit_module(
    &mut self,
    module_specifier: &ModuleSpecifier,
    source_file: SourceFile,
  ) -> Result<(), ErrBox> {
    let mut imports = vec![];
    let mut referenced_files = vec![];
    let mut lib_directives = vec![];
    let mut types_directives = vec![];
    let mut type_headers = vec![];

    // IMPORTANT: source_file.url might be different than requested
    // module_specifier because of HTTP redirects. In such
    // situation we add an "empty" ModuleGraphFile with 'redirect'
    // field set that will be later used in TS worker when building
    // map of available source file. It will perform substitution
    // for proper URL point to redirect target.
    if module_specifier.as_url() != &source_file.url {
      // TODO(bartlomieju): refactor, this is a band-aid
      self.graph.insert(
        module_specifier.to_string(),
        ModuleGraphFile {
          specifier: module_specifier.to_string(),
          url: module_specifier.to_string(),
          redirect: Some(source_file.url.to_string()),
          filename: source_file.filename.to_str().unwrap().to_string(),
          version_hash: checksum::gen(&[
            &source_file.source_code.as_bytes(),
            version::DENO.as_bytes(),
          ]),
          media_type: source_file.media_type,
          source_code: "".to_string(),
          imports: vec![],
          referenced_files: vec![],
          lib_directives: vec![],
          types_directives: vec![],
          type_headers: vec![],
        },
      );
    }

    let module_specifier = ModuleSpecifier::from(source_file.url.clone());
    let version_hash = checksum::gen(&[
      &source_file.source_code.as_bytes(),
      version::DENO.as_bytes(),
    ]);
    let source_code = source_file.source_code.to_string()?;

    if SUPPORTED_MEDIA_TYPES.contains(&source_file.media_type) {
      if let Some(types_specifier) = source_file.types_header {
        let type_header = ReferenceDescriptor {
          specifier: types_specifier.to_string(),
          resolved_specifier: ModuleSpecifier::resolve_import(
            &types_specifier,
            &module_specifier.to_string(),
          )?,
          kind: TsReferenceKind::Types,
          // TODO(bartlomieju): location is not needed in here and constructing
          // location by hand is bad
          location: Location {
            filename: module_specifier.to_string(),
            line: 0,
            col: 0,
          },
        };
        self.download_module(
          type_header.resolved_specifier.clone(),
          Some(module_specifier.clone()),
          None,
        )?;
        type_headers.push(type_header);
      }

      let (raw_imports, raw_refs) = pre_process_file(
        &module_specifier.to_string(),
        source_file.media_type,
        &source_code,
        self.analyze_dynamic_imports,
      )?;
      let (imports_, references) = resolve_imports_and_references(
        module_specifier.clone(),
        self.maybe_import_map.as_ref(),
        raw_imports,
        raw_refs,
      )?;

      for import_descriptor in imports_ {
        self.download_module(
          import_descriptor.resolved_specifier.clone(),
          Some(module_specifier.clone()),
          Some(import_descriptor.location.clone()),
        )?;

        if let Some(type_dir_url) =
          import_descriptor.resolved_type_directive.as_ref()
        {
          self.download_module(
            type_dir_url.clone(),
            Some(module_specifier.clone()),
            Some(import_descriptor.location.clone()),
          )?;
        }

        imports.push(import_descriptor);
      }

      for ref_descriptor in references {
        self.download_module(
          ref_descriptor.resolved_specifier.clone(),
          Some(module_specifier.clone()),
          Some(ref_descriptor.location.clone()),
        )?;

        match ref_descriptor.kind {
          TsReferenceKind::Lib => {
            lib_directives.push(ref_descriptor);
          }
          TsReferenceKind::Types => {
            types_directives.push(ref_descriptor);
          }
          TsReferenceKind::Path => {
            referenced_files.push(ref_descriptor);
          }
        }
      }
    }

    self.graph.insert(
      module_specifier.to_string(),
      ModuleGraphFile {
        specifier: module_specifier.to_string(),
        url: module_specifier.to_string(),
        redirect: None,
        version_hash,
        filename: source_file.filename.to_str().unwrap().to_string(),
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
  use crate::global_state::GlobalState;

  async fn build_graph(
    module_specifier: &ModuleSpecifier,
  ) -> Result<ModuleGraph, ErrBox> {
    let global_state = GlobalState::new(Default::default()).unwrap();
    let mut graph_loader = ModuleGraphLoader::new(
      global_state.file_fetcher.clone(),
      None,
      Permissions::allow_all(),
      false,
      false,
    );
    graph_loader.add_to_graph(&module_specifier, None).await?;
    Ok(graph_loader.get_graph())
  }

  // TODO(bartlomieju): this test is flaky, because it's using 019_media_types
  // file, reenable once Python server is replaced with Rust one.
  #[ignore]
  #[tokio::test]
  async fn source_graph_fetch() {
    let _http_server_guard = test_util::http_server();

    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/019_media_types.ts",
    )
    .unwrap();
    let graph = build_graph(&module_specifier)
      .await
      .expect("Failed to build graph");

    let a = graph
      .get("http://localhost:4545/cli/tests/019_media_types.ts")
      .unwrap();

    assert!(graph.contains_key(
      "http://localhost:4545/cli/tests/subdir/mt_text_ecmascript.j3.js"
    ));
    assert!(graph.contains_key(
      "http://localhost:4545/cli/tests/subdir/mt_video_vdn.t2.ts"
    ));
    assert!(graph.contains_key("http://localhost:4545/cli/tests/subdir/mt_application_x_typescript.t4.ts"));
    assert!(graph.contains_key(
      "http://localhost:4545/cli/tests/subdir/mt_video_mp2t.t3.ts"
    ));
    assert!(graph.contains_key("http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js"));
    assert!(graph.contains_key(
      "http://localhost:4545/cli/tests/subdir/mt_application_ecmascript.j2.js"
    ));
    assert!(graph.contains_key(
      "http://localhost:4545/cli/tests/subdir/mt_text_javascript.j1.js"
    ));
    assert!(graph.contains_key(
      "http://localhost:4545/cli/tests/subdir/mt_text_typescript.t1.ts"
    ));

    assert_eq!(
      serde_json::to_value(&a.imports).unwrap(),
      json!([
        {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_text_typescript.t1.ts",
          "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/mt_text_typescript.t1.ts",
          "typeDirective": null,
          "resolvedTypeDirective": null,
        },
        {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_video_vdn.t2.ts",
          "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/mt_video_vdn.t2.ts",
          "typeDirective": null,
          "resolvedTypeDirective": null,
        },
        {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_video_mp2t.t3.ts",
          "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/mt_video_mp2t.t3.ts",
          "typeDirective": null,
          "resolvedTypeDirective": null,
        },
        {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_application_x_typescript.t4.ts",
          "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/mt_application_x_typescript.t4.ts",
          "typeDirective": null,
          "resolvedTypeDirective": null,
        },
        {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_text_javascript.j1.js",
          "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/mt_text_javascript.j1.js",
          "typeDirective": null,
          "resolvedTypeDirective": null,
        },
        {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_application_ecmascript.j2.js",
          "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/mt_application_ecmascript.j2.js",
          "typeDirective": null,
          "resolvedTypeDirective": null,
        },
        {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_text_ecmascript.j3.js",
          "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/mt_text_ecmascript.j3.js",
          "typeDirective": null,
          "resolvedTypeDirective": null,
        },
        {
          "specifier": "http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js",
          "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js",
          "typeDirective": null,
          "resolvedTypeDirective": null,
        },
      ])
    );
  }

  #[tokio::test]
  async fn source_graph_type_references() {
    let _http_server_guard = test_util::http_server();

    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/type_definitions.ts",
    )
    .unwrap();

    let graph = build_graph(&module_specifier)
      .await
      .expect("Failed to build graph");

    eprintln!("json {:#?}", serde_json::to_value(&graph).unwrap());

    let a = graph
      .get("http://localhost:4545/cli/tests/type_definitions.ts")
      .unwrap();
    assert_eq!(
      serde_json::to_value(&a.imports).unwrap(),
      json!([
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
      ])
    );
    assert!(graph
      .contains_key("http://localhost:4545/cli/tests/type_definitions/foo.js"));
    assert!(graph.contains_key(
      "http://localhost:4545/cli/tests/type_definitions/foo.d.ts"
    ));
    assert!(graph.contains_key(
      "http://localhost:4545/cli/tests/type_definitions/fizz.js"
    ));
    assert!(graph.contains_key(
      "http://localhost:4545/cli/tests/type_definitions/fizz.d.ts"
    ));
    assert!(graph
      .contains_key("http://localhost:4545/cli/tests/type_definitions/qat.ts"));
  }

  #[tokio::test]
  async fn source_graph_type_references2() {
    let _http_server_guard = test_util::http_server();

    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/type_directives_02.ts",
    )
    .unwrap();

    let graph = build_graph(&module_specifier)
      .await
      .expect("Failed to build graph");

    eprintln!("{:#?}", serde_json::to_value(&graph).unwrap());

    let a = graph
      .get("http://localhost:4545/cli/tests/type_directives_02.ts")
      .unwrap();
    assert_eq!(
      serde_json::to_value(&a.imports).unwrap(),
      json!([
        {
          "specifier": "./subdir/type_reference.js",
          "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/type_reference.js",
          "typeDirective": null,
          "resolvedTypeDirective": null,
        }
      ])
    );

    assert!(graph.contains_key(
      "http://localhost:4545/cli/tests/subdir/type_reference.d.ts"
    ));

    let b = graph
      .get("http://localhost:4545/cli/tests/subdir/type_reference.js")
      .unwrap();
    assert_eq!(
      serde_json::to_value(&b.types_directives).unwrap(),
      json!([
        {
          "specifier": "./type_reference.d.ts",
          "resolvedSpecifier": "http://localhost:4545/cli/tests/subdir/type_reference.d.ts",
        }
      ])
    );
  }

  #[tokio::test]
  async fn source_graph_type_references3() {
    let _http_server_guard = test_util::http_server();

    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/type_directives_01.ts",
    )
    .unwrap();

    let graph = build_graph(&module_specifier)
      .await
      .expect("Failed to build graph");

    let ts = graph
      .get("http://localhost:4545/cli/tests/type_directives_01.ts")
      .unwrap();
    assert_eq!(
      serde_json::to_value(&ts.imports).unwrap(),
      json!([
        {
          "specifier": "http://127.0.0.1:4545/xTypeScriptTypes.js",
          "resolvedSpecifier": "http://127.0.0.1:4545/xTypeScriptTypes.js",
          "typeDirective": null,
          "resolvedTypeDirective": null,
        }
      ])
    );

    let headers = graph
      .get("http://127.0.0.1:4545/xTypeScriptTypes.js")
      .unwrap();
    assert_eq!(
      serde_json::to_value(&headers.type_headers).unwrap(),
      json!([
        {
          "specifier": "./xTypeScriptTypes.d.ts",
          "resolvedSpecifier": "http://127.0.0.1:4545/xTypeScriptTypes.d.ts"
        }
      ])
    );
  }

  #[tokio::test]
  async fn source_graph_different_langs() {
    let _http_server_guard = test_util::http_server();

    // ModuleGraphLoader was mistakenly parsing this file as TSX
    // https://github.com/denoland/deno/issues/5867

    let module_specifier = ModuleSpecifier::resolve_url_or_path(
      "http://localhost:4545/cli/tests/ts_with_generic.ts",
    )
    .unwrap();

    build_graph(&module_specifier)
      .await
      .expect("Failed to build graph");
  }
}

// TODO(bartlomieju): use baseline tests from TSC to ensure
// compatibility
#[test]
fn test_pre_process_file() {
  let source = r#"
// This comment is placed to make sure that directives are parsed
// even when they start on non-first line
  
/// <reference lib="dom" />
/// <reference types="./type_reference.d.ts" />
/// <reference path="./type_reference/dep.ts" />
// @deno-types="./type_definitions/foo.d.ts"
import { foo } from "./type_definitions/foo.js";
// @deno-types="./type_definitions/fizz.d.ts"
import "./type_definitions/fizz.js";

/// <reference path="./type_reference/dep2.ts" />

import * as qat from "./type_definitions/qat.ts";

console.log(foo);
console.log(fizz);
console.log(qat.qat);  
"#;

  let (imports, references) =
    pre_process_file("some/file.ts", MediaType::TypeScript, source, true)
      .expect("Failed to parse");

  assert_eq!(
    imports,
    vec![
      ImportDesc {
        specifier: "./type_definitions/foo.js".to_string(),
        deno_types: Some("./type_definitions/foo.d.ts".to_string()),
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 9,
          col: 0,
        },
      },
      ImportDesc {
        specifier: "./type_definitions/fizz.js".to_string(),
        deno_types: Some("./type_definitions/fizz.d.ts".to_string()),
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 11,
          col: 0,
        },
      },
      ImportDesc {
        specifier: "./type_definitions/qat.ts".to_string(),
        deno_types: None,
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 15,
          col: 0,
        },
      },
    ]
  );

  // According to TS docs (https://www.typescriptlang.org/docs/handbook/triple-slash-directives.html)
  // directives that are not at the top of the file are ignored, so only
  // 3 references should be captured instead of 4.
  assert_eq!(
    references,
    vec![
      TsReferenceDesc {
        specifier: "dom".to_string(),
        kind: TsReferenceKind::Lib,
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 5,
          col: 0,
        },
      },
      TsReferenceDesc {
        specifier: "./type_reference.d.ts".to_string(),
        kind: TsReferenceKind::Types,
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 6,
          col: 0,
        },
      },
      TsReferenceDesc {
        specifier: "./type_reference/dep.ts".to_string(),
        kind: TsReferenceKind::Path,
        location: Location {
          filename: "some/file.ts".to_string(),
          line: 7,
          col: 0,
        },
      },
    ]
  );
}
