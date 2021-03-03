// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::colors;
use crate::file_fetcher::File;
use crate::flags::Flags;
use crate::get_types;
use crate::media_type::MediaType;
use crate::module_graph;
use crate::program_state::ProgramState;
use crate::specifier_handler::FetchHandler;
use crate::write_json_to_stdout;
use crate::write_to_stdout_ignore_sigpipe;
use deno_core::error::AnyError;
use deno_core::futures::future::FutureExt;
use deno_core::futures::Future;
use deno_core::resolve_url_or_path;
use deno_doc as doc;
use deno_doc::parser::DocFileLoader;
use deno_runtime::permissions::Permissions;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use swc_ecmascript::parser::Syntax;

type DocResult = Result<(Syntax, String), doc::DocError>;

/// When parsing lib.deno.d.ts, only `DocParser::parse_source` is used,
/// which never even references the loader, so this is just a stub for that scenario.
///
/// TODO(Liamolucko): Refactor `deno_doc` so this isn't necessary.
struct StubDocLoader;

impl DocFileLoader for StubDocLoader {
  fn resolve(
    &self,
    _specifier: &str,
    _referrer: &str,
  ) -> Result<String, doc::DocError> {
    unreachable!()
  }

  fn load_source_code(
    &self,
    _specifier: &str,
  ) -> Pin<Box<dyn Future<Output = DocResult>>> {
    unreachable!()
  }
}

impl DocFileLoader for module_graph::Graph {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Result<String, doc::DocError> {
    let referrer =
      resolve_url_or_path(referrer).expect("Expected valid specifier");
    match self.resolve(specifier, &referrer, true) {
      Ok(specifier) => Ok(specifier.to_string()),
      Err(e) => Err(doc::DocError::Resolve(e.to_string())),
    }
  }

  fn load_source_code(
    &self,
    specifier: &str,
  ) -> Pin<Box<dyn Future<Output = DocResult>>> {
    let specifier =
      resolve_url_or_path(specifier).expect("Expected valid specifier");
    let source = self.get_source(&specifier).expect("Unknown dependency");
    let media_type =
      self.get_media_type(&specifier).expect("Unknown media type");
    let syntax = ast::get_syntax(&media_type);
    async move { Ok((syntax, source)) }.boxed_local()
  }
}

pub async fn print_docs(
  flags: Flags,
  source_file: Option<String>,
  json: bool,
  maybe_filter: Option<String>,
  private: bool,
) -> Result<(), AnyError> {
  let program_state = ProgramState::build(flags.clone()).await?;
  let source_file = source_file.unwrap_or_else(|| "--builtin".to_string());

  let parse_result = if source_file == "--builtin" {
    let loader = Box::new(StubDocLoader);
    let doc_parser = doc::DocParser::new(loader, private);

    let syntax = ast::get_syntax(&MediaType::Dts);
    doc_parser.parse_source(
      "lib.deno.d.ts",
      syntax,
      get_types(flags.unstable).as_str(),
    )
  } else {
    let module_specifier = resolve_url_or_path(&source_file).unwrap();

    // If the root module has external types, the module graph won't redirect it,
    // so instead create a dummy file which exports everything from the actual file being documented.
    let root_specifier = resolve_url_or_path("./$deno$doc.ts").unwrap();
    let root = File {
      local: PathBuf::from("./$deno$doc.ts"),
      maybe_types: None,
      media_type: MediaType::TypeScript,
      source: format!("export * from \"{}\";", module_specifier),
      specifier: root_specifier.clone(),
    };

    // Save our fake file into file fetcher cache.
    program_state.file_fetcher.insert_cached(root);

    let handler = Arc::new(Mutex::new(FetchHandler::new(
      &program_state,
      Permissions::allow_all(),
    )?));
    let mut builder = module_graph::GraphBuilder::new(
      handler,
      program_state.maybe_import_map.clone(),
      program_state.lockfile.clone(),
    );
    builder.add(&root_specifier, false).await?;
    let graph = builder.get_graph();

    let doc_parser = doc::DocParser::new(Box::new(graph), private);
    doc_parser
      .parse_with_reexports(root_specifier.as_str())
      .await
  };

  let mut doc_nodes = match parse_result {
    Ok(nodes) => nodes,
    Err(e) => {
      eprintln!("{}", e);
      std::process::exit(1);
    }
  };

  if json {
    write_json_to_stdout(&doc_nodes)
  } else {
    doc_nodes.retain(|doc_node| doc_node.kind != doc::DocNodeKind::Import);
    let details = if let Some(filter) = maybe_filter {
      let nodes =
        doc::find_nodes_by_name_recursively(doc_nodes, filter.clone());
      if nodes.is_empty() {
        eprintln!("Node {} was not found!", filter);
        std::process::exit(1);
      }
      format!(
        "{}",
        doc::DocPrinter::new(&nodes, colors::use_color(), private)
      )
    } else {
      format!(
        "{}",
        doc::DocPrinter::new(&doc_nodes, colors::use_color(), private)
      )
    };

    write_to_stdout_ignore_sigpipe(details.as_bytes()).map_err(AnyError::from)
  }
}
