// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::file_fetcher::File;
use crate::flags::DocFlags;
use crate::flags::Flags;
use crate::get_types;
use crate::proc_state::ProcState;
use crate::write_json_to_stdout;
use crate::write_to_stdout_ignore_sigpipe;
use deno_ast::MediaType;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::future::FutureExt;
use deno_core::resolve_url_or_path;
use deno_doc as doc;
use deno_graph::create_graph;
use deno_graph::source::LoadFuture;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_graph::source::ResolveResponse;
use deno_graph::source::Resolver;
use deno_graph::ModuleKind;
use deno_graph::ModuleSpecifier;
use deno_runtime::permissions::Permissions;
use import_map::ImportMap;
use std::path::PathBuf;
use std::sync::Arc;

struct StubDocLoader;

impl Loader for StubDocLoader {
  fn load(
    &mut self,
    _specifier: &ModuleSpecifier,
    _is_dynamic: bool,
  ) -> LoadFuture {
    Box::pin(future::ready(Ok(None)))
  }
}

#[derive(Debug)]
struct DocResolver {
  import_map: Option<Arc<ImportMap>>,
}

impl Resolver for DocResolver {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> ResolveResponse {
    if let Some(import_map) = &self.import_map {
      return match import_map.resolve(specifier, referrer) {
        Ok(specifier) => ResolveResponse::Specifier(specifier),
        Err(err) => ResolveResponse::Err(err.into()),
      };
    }

    match deno_core::resolve_import(specifier, referrer.as_str()) {
      Ok(specifier) => ResolveResponse::Specifier(specifier),
      Err(err) => ResolveResponse::Err(err.into()),
    }
  }
}

struct DocLoader {
  ps: ProcState,
}

impl Loader for DocLoader {
  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    _is_dynamic: bool,
  ) -> LoadFuture {
    let specifier = specifier.clone();
    let ps = self.ps.clone();
    async move {
      ps.file_fetcher
        .fetch(&specifier, &mut Permissions::allow_all())
        .await
        .map(|file| {
          Some(LoadResponse::Module {
            specifier,
            content: file.source.clone(),
            maybe_headers: file.maybe_headers,
          })
        })
    }
    .boxed_local()
  }
}

pub async fn print_docs(
  flags: Flags,
  doc_flags: DocFlags,
) -> Result<(), AnyError> {
  let ps = ProcState::build(Arc::new(flags)).await?;
  let source_file = doc_flags
    .source_file
    .unwrap_or_else(|| "--builtin".to_string());
  let source_parser = deno_graph::DefaultSourceParser::new();

  let parse_result = if source_file == "--builtin" {
    let mut loader = StubDocLoader;
    let source_file_specifier =
      ModuleSpecifier::parse("deno://lib.deno.d.ts").unwrap();
    let graph = create_graph(
      vec![(source_file_specifier.clone(), ModuleKind::Esm)],
      false,
      None,
      &mut loader,
      None,
      None,
      None,
      None,
    )
    .await;
    let doc_parser =
      doc::DocParser::new(graph, doc_flags.private, &source_parser);
    doc_parser.parse_source(
      &source_file_specifier,
      MediaType::Dts,
      Arc::new(get_types(ps.flags.unstable)),
    )
  } else {
    let module_specifier = resolve_url_or_path(&source_file)?;

    // If the root module has external types, the module graph won't redirect it,
    // so instead create a dummy file which exports everything from the actual file being documented.
    let root_specifier = resolve_url_or_path("./$deno$doc.ts").unwrap();
    let root = File {
      local: PathBuf::from("./$deno$doc.ts"),
      maybe_types: None,
      media_type: MediaType::TypeScript,
      source: Arc::new(format!("export * from \"{}\";", module_specifier)),
      specifier: root_specifier.clone(),
      maybe_headers: None,
    };

    // Save our fake file into file fetcher cache.
    ps.file_fetcher.insert_cached(root);

    let mut loader = DocLoader { ps: ps.clone() };
    let resolver = DocResolver {
      import_map: ps.maybe_import_map.clone(),
    };
    let graph = create_graph(
      vec![(root_specifier.clone(), ModuleKind::Esm)],
      false,
      None,
      &mut loader,
      Some(&resolver),
      None,
      None,
      None,
    )
    .await;
    let doc_parser =
      doc::DocParser::new(graph, doc_flags.private, &source_parser);
    doc_parser.parse_with_reexports(&root_specifier)
  };

  let mut doc_nodes = match parse_result {
    Ok(nodes) => nodes,
    Err(e) => {
      eprintln!("{}", e);
      std::process::exit(1);
    }
  };

  if doc_flags.json {
    write_json_to_stdout(&doc_nodes)
  } else {
    doc_nodes.retain(|doc_node| doc_node.kind != doc::DocNodeKind::Import);
    let details = if let Some(filter) = doc_flags.filter {
      let nodes =
        doc::find_nodes_by_name_recursively(doc_nodes, filter.clone());
      if nodes.is_empty() {
        eprintln!("Node {} was not found!", filter);
        std::process::exit(1);
      }
      format!(
        "{}",
        doc::DocPrinter::new(&nodes, colors::use_color(), doc_flags.private)
      )
    } else {
      format!(
        "{}",
        doc::DocPrinter::new(
          &doc_nodes,
          colors::use_color(),
          doc_flags.private
        )
      )
    };

    write_to_stdout_ignore_sigpipe(details.as_bytes()).map_err(AnyError::from)
  }
}
