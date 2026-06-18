// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

use deno_ast::MediaType;
use deno_core::ModuleSpecifier;
use deno_core::anyhow;
use deno_core::error::AnyError;
use deno_graph::GraphKind;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_path_util::resolve_url_or_path;

use crate::args::Flags;
use crate::args::TypesFlags;
use crate::factory::CliFactory;
use crate::graph_util::BuildGraphWithNpmOptions;
use crate::tsc;
use crate::util::display;

pub async fn types(
  flags: Arc<Flags>,
  types_flags: TypesFlags,
) -> Result<(), AnyError> {
  // No specifiers: keep the historical behavior of printing Deno's built-in
  // runtime declarations.
  if types_flags.specifiers.is_empty() {
    let types = tsc::get_types_declaration_file_text();
    return display::write_to_stdout_ignore_sigpipe(types.as_bytes())
      .map_err(Into::into);
  }

  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;

  // Resolve each specifier argument. `resolve_url_or_path` handles jsr:, npm:,
  // http(s): and bare file paths.
  let roots = types_flags
    .specifiers
    .iter()
    .map(|s| resolve_url_or_path(s, cli_options.initial_cwd()))
    .collect::<Result<Vec<_>, _>>()?;

  // Build a graph for the roots so jsr:/npm: specifiers get resolved to their
  // underlying modules and all dependencies are available to TSC.
  let mut graph = ModuleGraph::new(GraphKind::All);
  let module_graph_builder = factory.module_graph_builder().await?;
  module_graph_builder
    .build_graph_roots_with_npm_resolution(
      &mut graph,
      roots,
      BuildGraphWithNpmOptions {
        is_dynamic: false,
        loader: None,
        npm_caching: cli_options.default_npm_caching_strategy(),
      },
    )
    .await?;
  graph.valid()?;

  // Building the graph may have rewritten the roots (e.g. jsr:/npm: to the
  // underlying file:/https: module), so read them back from the graph and pair
  // each with its media type for TSC.
  let mut root_names: Vec<(ModuleSpecifier, MediaType)> = Vec::new();
  for root in &graph.roots {
    let resolved = graph.resolve(root);
    match graph.get(resolved) {
      Some(Module::Js(module)) => {
        root_names.push((module.specifier.clone(), module.media_type));
      }
      _ => {
        root_names
          .push((resolved.clone(), MediaType::from_specifier(resolved)));
      }
    }
  }

  let type_checker = factory.type_checker().await?;
  let result = type_checker.emit_declarations(
    Arc::new(graph),
    root_names,
    cli_options.ts_type_lib_window(),
  )?;

  if result.diagnostics.has_diagnostic() {
    anyhow::bail!("Type checking failed:\n{}", result.diagnostics);
  }

  // Concatenate the emitted `.d.ts` files and print to stdout. Each file is
  // prefixed with a comment noting its source specifier.
  let mut output = String::new();
  for (file_name, content) in &result.emitted_files {
    output.push_str("// ");
    output.push_str(file_name);
    output.push('\n');
    output.push_str(content);
    if !content.ends_with('\n') {
      output.push('\n');
    }
  }

  display::write_to_stdout_ignore_sigpipe(output.as_bytes())?;
  Ok(())
}
