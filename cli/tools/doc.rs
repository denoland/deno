// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::DocFlags;
use crate::args::DocSourceFileFlag;
use crate::args::Flags;
use crate::colors;
use crate::display::write_json_to_stdout;
use crate::display::write_to_stdout_ignore_sigpipe;
use crate::factory::CliFactory;
use crate::graph_util::graph_lock_or_exit;
use crate::graph_util::CreateGraphOptions;
use crate::tsc::get_types_declaration_file_text;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_doc as doc;
use deno_graph::CapturingModuleParser;
use deno_graph::DefaultParsedSourceStore;
use deno_graph::GraphKind;
use deno_graph::ModuleSpecifier;

pub async fn print_docs(
  flags: Flags,
  doc_flags: DocFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags).await?;
  let cli_options = factory.cli_options();
  let module_info_cache = factory.module_info_cache()?;
  let source_parser = deno_graph::DefaultModuleParser::new_for_analysis();
  let store = DefaultParsedSourceStore::default();
  let analyzer =
    module_info_cache.as_module_analyzer(Some(&source_parser), &store);
  let capturing_parser =
    CapturingModuleParser::new(Some(&source_parser), &store);

  let mut doc_nodes = match doc_flags.source_files {
    DocSourceFileFlag::Builtin => {
      let source_file_specifier =
        ModuleSpecifier::parse("internal://lib.deno.d.ts").unwrap();
      let content = get_types_declaration_file_text(cli_options.unstable());
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
      let mut graph = deno_graph::ModuleGraph::new(GraphKind::TypesOnly);
      graph
        .build(
          vec![source_file_specifier.clone()],
          &mut loader,
          deno_graph::BuildOptions {
            module_analyzer: Some(&analyzer),
            ..Default::default()
          },
        )
        .await;
      let doc_parser =
        doc::DocParser::new(&graph, doc_flags.private, capturing_parser)?;
      doc_parser.parse_module(&source_file_specifier)?.definitions
    }
    DocSourceFileFlag::Paths(source_files) => {
      let module_graph_builder = factory.module_graph_builder().await?;
      let maybe_lockfile = factory.maybe_lockfile();

      let module_specifiers: Result<Vec<ModuleSpecifier>, AnyError> =
        source_files
          .iter()
          .map(|source_file| {
            Ok(resolve_url_or_path(source_file, cli_options.initial_cwd())?)
          })
          .collect();
      let module_specifiers = module_specifiers?;
      let mut loader = module_graph_builder.create_graph_loader();
      let graph = module_graph_builder
        .create_graph_with_options(CreateGraphOptions {
          graph_kind: GraphKind::TypesOnly,
          roots: module_specifiers.clone(),
          loader: &mut loader,
          analyzer: &analyzer,
        })
        .await?;

      if let Some(lockfile) = maybe_lockfile {
        graph_lock_or_exit(&graph, &mut lockfile.lock());
      }

      let doc_parser =
        doc::DocParser::new(&graph, doc_flags.private, capturing_parser)?;

      let mut doc_nodes = vec![];

      for module_specifier in module_specifiers {
        let nodes = doc_parser.parse_with_reexports(&module_specifier)?;
        doc_nodes.extend_from_slice(&nodes);
      }

      doc_nodes
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
