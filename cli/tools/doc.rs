// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::args::DocFlags;
use crate::args::Flags;
use crate::colors;
use crate::file_fetcher::File;
use crate::get_types;
use crate::proc_state::ProcState;
use crate::write_json_to_stdout;
use crate::write_to_stdout_ignore_sigpipe;
use deno_ast::MediaType;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_doc as doc;
use deno_graph::ModuleKind;
use deno_graph::ModuleSpecifier;
use std::path::PathBuf;

pub async fn print_docs(
  flags: Flags,
  doc_flags: DocFlags,
) -> Result<(), AnyError> {
  let ps = ProcState::build(flags).await?;
  let source_file = doc_flags
    .source_file
    .unwrap_or_else(|| "--builtin".to_string());

  let mut doc_nodes = if source_file == "--builtin" {
    let source_file_specifier =
      ModuleSpecifier::parse("deno://lib.deno.d.ts").unwrap();
    let content = get_types(ps.options.unstable());
    let mut loader = deno_graph::source::MemoryLoader::new(
      vec![(
        source_file_specifier.to_string(),
        deno_graph::source::Source::Module {
          specifier: source_file_specifier.to_string(),
          content,
          maybe_headers: None,
        },
      )],
      Vec::new(),
    );
    let analyzer = deno_graph::CapturingModuleAnalyzer::default();
    let graph = deno_graph::create_graph(
      vec![(source_file_specifier.clone(), ModuleKind::Esm)],
      false,
      None,
      &mut loader,
      None,
      None,
      Some(&analyzer),
      None,
    )
    .await;
    let doc_parser = doc::DocParser::new(graph, doc_flags.private, &analyzer);
    doc_parser.parse_module(&source_file_specifier)?.definitions
  } else {
    let module_specifier = resolve_url_or_path(&source_file)?;

    // If the root module has external types, the module graph won't redirect it,
    // so instead create a dummy file which exports everything from the actual file being documented.
    let root_specifier = resolve_url_or_path("./$deno$doc.ts").unwrap();
    let root = File {
      local: PathBuf::from("./$deno$doc.ts"),
      maybe_types: None,
      media_type: MediaType::TypeScript,
      source: format!("export * from \"{}\";", module_specifier).into(),
      specifier: root_specifier.clone(),
      maybe_headers: None,
    };

    // Save our fake file into file fetcher cache.
    ps.file_fetcher.insert_cached(root);

    let graph = ps
      .create_graph(vec![(root_specifier.clone(), ModuleKind::Esm)])
      .await?;
    let store = ps.parsed_source_cache.as_store();
    let doc_parser = doc::DocParser::new(graph, doc_flags.private, &*store);
    doc_parser.parse_with_reexports(&root_specifier)?
  };

  if doc_flags.json {
    write_json_to_stdout(&doc_nodes)
  } else {
    doc_nodes.retain(|doc_node| doc_node.kind != doc::DocNodeKind::Import);
    let details = if let Some(filter) = doc_flags.filter {
      let nodes =
        doc::find_nodes_by_name_recursively(doc_nodes, filter.clone());
      if nodes.is_empty() {
        bail!("Node {} was not found!", filter);
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
