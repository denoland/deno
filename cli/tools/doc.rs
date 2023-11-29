// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::CliOptions;
use crate::args::DocFlags;
use crate::args::DocHtmlFlag;
use crate::args::DocSourceFileFlag;
use crate::args::Flags;
use crate::colors;
use crate::display::write_json_to_stdout;
use crate::display::write_to_stdout_ignore_sigpipe;
use crate::factory::CliFactory;
use crate::graph_util::graph_lock_or_exit;
use crate::graph_util::CreateGraphOptions;
use crate::tsc::get_types_declaration_file_text;
use crate::util::glob::expand_globs;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::resolve_url_or_path;
use deno_doc as doc;
use deno_graph::CapturingModuleParser;
use deno_graph::DefaultParsedSourceStore;
use deno_graph::GraphKind;
use deno_graph::ModuleAnalyzer;
use deno_graph::ModuleSpecifier;
use doc::DocDiagnostic;
use indexmap::IndexMap;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

async fn generate_doc_nodes_for_builtin_types(
  doc_flags: DocFlags,
  cli_options: &Arc<CliOptions>,
  capturing_parser: CapturingModuleParser<'_>,
  analyzer: &dyn ModuleAnalyzer,
) -> Result<IndexMap<ModuleSpecifier, Vec<doc::DocNode>>, AnyError> {
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
        module_analyzer: Some(analyzer),
        ..Default::default()
      },
    )
    .await;
  let doc_parser = doc::DocParser::new(
    &graph,
    capturing_parser,
    doc::DocParserOptions {
      diagnostics: false,
      private: doc_flags.private,
    },
  )?;
  let nodes = doc_parser.parse_module(&source_file_specifier)?.definitions;

  Ok(IndexMap::from([(source_file_specifier, nodes)]))
}

pub async fn doc(flags: Flags, doc_flags: DocFlags) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags).await?;
  let cli_options = factory.cli_options();
  let module_info_cache = factory.module_info_cache()?;
  let source_parser = deno_graph::DefaultModuleParser::new_for_analysis();
  let store = DefaultParsedSourceStore::default();
  let analyzer =
    module_info_cache.as_module_analyzer(Some(&source_parser), &store);
  let capturing_parser =
    CapturingModuleParser::new(Some(&source_parser), &store);

  let doc_nodes_by_url = match doc_flags.source_files {
    DocSourceFileFlag::Builtin => {
      generate_doc_nodes_for_builtin_types(
        doc_flags.clone(),
        cli_options,
        capturing_parser,
        &analyzer,
      )
      .await?
    }
    DocSourceFileFlag::Paths(ref source_files) => {
      let module_graph_builder = factory.module_graph_builder().await?;
      let maybe_lockfile = factory.maybe_lockfile();

      let expanded_globs =
        expand_globs(source_files.iter().map(PathBuf::from).collect())?;
      let module_specifiers: Result<Vec<ModuleSpecifier>, AnyError> =
        expanded_globs
          .iter()
          .map(|source_file| {
            Ok(resolve_url_or_path(
              &source_file.to_string_lossy(),
              cli_options.initial_cwd(),
            )?)
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

      let doc_parser = doc::DocParser::new(
        &graph,
        capturing_parser,
        doc::DocParserOptions {
          private: doc_flags.private,
          diagnostics: doc_flags.lint,
        },
      )?;

      let mut doc_nodes_by_url =
        IndexMap::with_capacity(module_specifiers.len());

      for module_specifier in module_specifiers {
        let nodes = doc_parser.parse_with_reexports(&module_specifier)?;
        doc_nodes_by_url.insert(module_specifier, nodes);
      }

      if doc_flags.lint {
        let diagnostics = doc_parser.take_diagnostics();
        check_diagnostics(&diagnostics)?;
      }

      doc_nodes_by_url
    }
  };

  if let Some(html_options) = doc_flags.html {
    generate_docs_directory(&doc_nodes_by_url, html_options)
      .boxed_local()
      .await
  } else {
    let modules_len = doc_nodes_by_url.len();
    let doc_nodes =
      doc_nodes_by_url.into_values().flatten().collect::<Vec<_>>();

    if doc_flags.json {
      write_json_to_stdout(&doc_nodes)
    } else if doc_flags.lint {
      // don't output docs if running with only the --lint flag
      log::info!(
        "Checked {} file{}",
        modules_len,
        if modules_len == 1 { "" } else { "s" }
      );
      Ok(())
    } else {
      print_docs_to_stdout(doc_flags, doc_nodes)
    }
  }
}

async fn generate_docs_directory(
  doc_nodes_by_url: &IndexMap<ModuleSpecifier, Vec<doc::DocNode>>,
  html_options: DocHtmlFlag,
) -> Result<(), AnyError> {
  let cwd = std::env::current_dir().context("Failed to get CWD")?;
  let output_dir_resolved = cwd.join(&html_options.output);

  let options = deno_doc::html::GenerateOptions {
    package_name: Some(html_options.name),
    main_entrypoint: None,
    global_symbols: Default::default(),
    global_symbol_href_resolver: Rc::new(|_, _| String::new()),
    url_resolver: Rc::new(deno_doc::html::default_url_resolver),
  };

  let files = deno_doc::html::generate(options, doc_nodes_by_url)
    .context("Failed to generate HTML documentation")?;

  let path = &output_dir_resolved;
  let _ = std::fs::remove_dir_all(path);
  std::fs::create_dir(path)
    .with_context(|| format!("Failed to create directory {:?}", path))?;

  let no_of_files = files.len();
  for (name, content) in files {
    let this_path = path.join(name);
    let prefix = this_path.parent().with_context(|| {
      format!("Failed to get parent path for {:?}", this_path)
    })?;
    std::fs::create_dir_all(prefix)
      .with_context(|| format!("Failed to create directory {:?}", prefix))?;
    std::fs::write(&this_path, content)
      .with_context(|| format!("Failed to write file {:?}", this_path))?;
  }

  log::info!(
    "{}",
    colors::green(format!(
      "Written {} files to {:?}",
      no_of_files, html_options.output
    ))
  );
  Ok(())
}

fn print_docs_to_stdout(
  doc_flags: DocFlags,
  mut doc_nodes: Vec<deno_doc::DocNode>,
) -> Result<(), AnyError> {
  doc_nodes.retain(|doc_node| doc_node.kind != doc::DocNodeKind::Import);
  let details = if let Some(filter) = doc_flags.filter {
    let nodes = doc::find_nodes_by_name_recursively(doc_nodes, filter.clone());
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
      doc::DocPrinter::new(&doc_nodes, colors::use_color(), doc_flags.private)
    )
  };

  write_to_stdout_ignore_sigpipe(details.as_bytes()).map_err(AnyError::from)
}

fn check_diagnostics(diagnostics: &[DocDiagnostic]) -> Result<(), AnyError> {
  if diagnostics.is_empty() {
    return Ok(());
  }

  // group by location then by line (sorted) then column (sorted)
  let mut diagnostic_groups = IndexMap::new();
  for diagnostic in diagnostics {
    diagnostic_groups
      .entry(diagnostic.location.filename.clone())
      .or_insert_with(BTreeMap::new)
      .entry(diagnostic.location.line)
      .or_insert_with(BTreeMap::new)
      .entry(diagnostic.location.col)
      .or_insert_with(Vec::new)
      .push(diagnostic);
  }

  for (filename, diagnostics_by_lc) in diagnostic_groups {
    for (line, diagnostics_by_col) in diagnostics_by_lc {
      for (col, diagnostics) in diagnostics_by_col {
        for diagnostic in diagnostics {
          log::warn!("{}", diagnostic.message());
        }
        log::warn!(
          "    at {}:{}:{}\n",
          colors::cyan(filename.as_str()),
          colors::yellow(&line.to_string()),
          colors::yellow(&(col + 1).to_string())
        )
      }
    }
  }
  bail!(
    "Found {} documentation lint error{}.",
    colors::bold(diagnostics.len().to_string()),
    if diagnostics.len() == 1 { "" } else { "s" }
  );
}
