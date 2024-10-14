// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::args::DocFlags;
use crate::args::DocHtmlFlag;
use crate::args::DocSourceFileFlag;
use crate::args::Flags;
use crate::colors;
use crate::display;
use crate::factory::CliFactory;
use crate::graph_util::graph_exit_integrity_errors;
use crate::graph_util::graph_walk_errors;
use crate::graph_util::GraphWalkErrorsOptions;
use crate::tsc::get_types_declaration_file_text;
use crate::util::fs::collect_specifiers;
use deno_ast::diagnostics::Diagnostic;
use deno_config::glob::FilePatterns;
use deno_config::glob::PathOrPatternSet;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_doc as doc;
use deno_doc::html::UrlResolveKind;
use deno_graph::source::NullFileSystem;
use deno_graph::GraphKind;
use deno_graph::ModuleAnalyzer;
use deno_graph::ModuleParser;
use deno_graph::ModuleSpecifier;
use doc::html::ShortPath;
use doc::DocDiagnostic;
use indexmap::IndexMap;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

const JSON_SCHEMA_VERSION: u8 = 1;

async fn generate_doc_nodes_for_builtin_types(
  doc_flags: DocFlags,
  parser: &dyn ModuleParser,
  analyzer: &dyn ModuleAnalyzer,
) -> Result<IndexMap<ModuleSpecifier, Vec<doc::DocNode>>, AnyError> {
  let source_file_specifier =
    ModuleSpecifier::parse("file:///lib.deno.d.ts").unwrap();
  let content = get_types_declaration_file_text();
  let loader = deno_graph::source::MemoryLoader::new(
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
      &loader,
      deno_graph::BuildOptions {
        imports: Vec::new(),
        is_dynamic: false,
        passthrough_jsr_specifiers: false,
        executor: Default::default(),
        file_system: &NullFileSystem,
        jsr_url_provider: Default::default(),
        locker: None,
        module_analyzer: analyzer,
        npm_resolver: None,
        reporter: None,
        resolver: None,
      },
    )
    .await;
  let doc_parser = doc::DocParser::new(
    &graph,
    parser,
    doc::DocParserOptions {
      diagnostics: false,
      private: doc_flags.private,
    },
  )?;
  let nodes = doc_parser.parse_module(&source_file_specifier)?.definitions;

  Ok(IndexMap::from([(source_file_specifier, nodes)]))
}

pub async fn doc(
  flags: Arc<Flags>,
  doc_flags: DocFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let module_info_cache = factory.module_info_cache()?;
  let parsed_source_cache = factory.parsed_source_cache();
  let capturing_parser = parsed_source_cache.as_capturing_parser();
  let analyzer = module_info_cache.as_module_analyzer(parsed_source_cache);

  let doc_nodes_by_url = match doc_flags.source_files {
    DocSourceFileFlag::Builtin => {
      generate_doc_nodes_for_builtin_types(
        doc_flags.clone(),
        &capturing_parser,
        &analyzer,
      )
      .await?
    }
    DocSourceFileFlag::Paths(ref source_files) => {
      let module_graph_creator = factory.module_graph_creator().await?;
      let fs = factory.fs();

      let module_specifiers = collect_specifiers(
        FilePatterns {
          base: cli_options.initial_cwd().to_path_buf(),
          include: Some(
            PathOrPatternSet::from_include_relative_path_or_patterns(
              cli_options.initial_cwd(),
              source_files,
            )?,
          ),
          exclude: Default::default(),
        },
        cli_options.vendor_dir_path().map(ToOwned::to_owned),
        |_| true,
      )?;
      let graph = module_graph_creator
        .create_graph(GraphKind::TypesOnly, module_specifiers.clone())
        .await?;

      graph_exit_integrity_errors(&graph);
      let errors = graph_walk_errors(
        &graph,
        fs,
        &module_specifiers,
        GraphWalkErrorsOptions {
          check_js: false,
          kind: GraphKind::TypesOnly,
        },
      );
      for error in errors {
        log::warn!("{} {}", colors::yellow("Warning"), error);
      }

      let doc_parser = doc::DocParser::new(
        &graph,
        &capturing_parser,
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

  if let Some(html_options) = &doc_flags.html {
    let deno_ns = if doc_flags.source_files != DocSourceFileFlag::Builtin {
      let deno_ns = generate_doc_nodes_for_builtin_types(
        doc_flags.clone(),
        &capturing_parser,
        &analyzer,
      )
      .await?;
      let (_, deno_ns) = deno_ns.into_iter().next().unwrap();

      let short_path = Rc::new(ShortPath::new(
        ModuleSpecifier::parse("file:///lib.deno.d.ts").unwrap(),
        None,
        None,
        None,
      ));

      deno_doc::html::compute_namespaced_symbols(
        &deno_ns
          .into_iter()
          .map(|node| deno_doc::html::DocNodeWithContext {
            origin: short_path.clone(),
            ns_qualifiers: Rc::new([]),
            kind_with_drilldown:
              deno_doc::html::DocNodeKindWithDrilldown::Other(node.kind()),
            inner: Rc::new(node),
            drilldown_name: None,
            parent: None,
          })
          .collect::<Vec<_>>(),
      )
    } else {
      Default::default()
    };

    let rewrite_map =
      if let Some(config_file) = cli_options.start_dir.maybe_deno_json() {
        let config = config_file.to_exports_config()?;

        let rewrite_map = config
          .clone()
          .into_map()
          .into_keys()
          .map(|key| {
            Ok((
              config.get_resolved(&key)?.unwrap(),
              key
                .strip_prefix('.')
                .unwrap_or(&key)
                .strip_prefix('/')
                .unwrap_or(&key)
                .to_owned(),
            ))
          })
          .collect::<Result<IndexMap<_, _>, AnyError>>()?;

        Some(rewrite_map)
      } else {
        None
      };

    generate_docs_directory(
      doc_nodes_by_url,
      html_options,
      deno_ns,
      rewrite_map,
    )
  } else {
    let modules_len = doc_nodes_by_url.len();
    let doc_nodes =
      doc_nodes_by_url.into_values().flatten().collect::<Vec<_>>();

    if doc_flags.json {
      let json_output = serde_json::json!({
        "version": JSON_SCHEMA_VERSION,
        "nodes": &doc_nodes
      });
      display::write_json_to_stdout(&json_output)
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

struct DocResolver {
  deno_ns: std::collections::HashMap<Vec<String>, Option<Rc<ShortPath>>>,
  strip_trailing_html: bool,
}

impl deno_doc::html::HrefResolver for DocResolver {
  fn resolve_path(
    &self,
    current: UrlResolveKind,
    target: UrlResolveKind,
  ) -> String {
    let path = deno_doc::html::href_path_resolve(current, target);
    if self.strip_trailing_html {
      if let Some(path) = path
        .strip_suffix("index.html")
        .or_else(|| path.strip_suffix(".html"))
      {
        return path.to_owned();
      }
    }

    path
  }

  fn resolve_global_symbol(&self, symbol: &[String]) -> Option<String> {
    if self.deno_ns.contains_key(symbol) {
      Some(format!(
        "https://deno.land/api@v{}?s={}",
        env!("CARGO_PKG_VERSION"),
        symbol.join(".")
      ))
    } else {
      None
    }
  }

  fn resolve_import_href(
    &self,
    symbol: &[String],
    src: &str,
  ) -> Option<String> {
    let mut url = ModuleSpecifier::parse(src).ok()?;

    if url.domain() == Some("deno.land") {
      url.set_query(Some(&format!("s={}", symbol.join("."))));
      return Some(url.to_string());
    }

    None
  }

  fn resolve_usage(&self, current_resolve: UrlResolveKind) -> Option<String> {
    current_resolve.get_file().map(|file| file.path.to_string())
  }

  fn resolve_source(&self, location: &deno_doc::Location) -> Option<String> {
    Some(location.filename.to_string())
  }

  fn resolve_external_jsdoc_module(
    &self,
    module: &str,
    _symbol: Option<&str>,
  ) -> Option<(String, String)> {
    if let Ok(url) = deno_core::url::Url::parse(module) {
      match url.scheme() {
        "npm" => {
          let res =
            deno_semver::npm::NpmPackageReqReference::from_str(module).ok()?;
          let name = &res.req().name;
          Some((
            format!("https://www.npmjs.com/package/{name}"),
            name.to_owned(),
          ))
        }
        "jsr" => {
          let res =
            deno_semver::jsr::JsrPackageReqReference::from_str(module).ok()?;
          let name = &res.req().name;
          Some((format!("https://jsr.io/{name}"), name.to_owned()))
        }
        _ => None,
      }
    } else {
      None
    }
  }
}

struct DenoDocResolver(bool);

impl deno_doc::html::HrefResolver for DenoDocResolver {
  fn resolve_path(
    &self,
    current: UrlResolveKind,
    target: UrlResolveKind,
  ) -> String {
    let path = deno_doc::html::href_path_resolve(current, target);
    if self.0 {
      if let Some(path) = path
        .strip_suffix("index.html")
        .or_else(|| path.strip_suffix(".html"))
      {
        return path.to_owned();
      }
    }

    path
  }

  fn resolve_global_symbol(&self, _symbol: &[String]) -> Option<String> {
    None
  }

  fn resolve_import_href(
    &self,
    _symbol: &[String],
    _src: &str,
  ) -> Option<String> {
    None
  }

  fn resolve_usage(&self, _current_resolve: UrlResolveKind) -> Option<String> {
    None
  }

  fn resolve_source(&self, _location: &deno_doc::Location) -> Option<String> {
    None
  }

  fn resolve_external_jsdoc_module(
    &self,
    _module: &str,
    _symbol: Option<&str>,
  ) -> Option<(String, String)> {
    None
  }
}

struct NodeDocResolver(bool);

impl deno_doc::html::HrefResolver for NodeDocResolver {
  fn resolve_path(
    &self,
    current: UrlResolveKind,
    target: UrlResolveKind,
  ) -> String {
    let path = deno_doc::html::href_path_resolve(current, target);
    if self.0 {
      if let Some(path) = path
        .strip_suffix("index.html")
        .or_else(|| path.strip_suffix(".html"))
      {
        return path.to_owned();
      }
    }

    path
  }

  fn resolve_global_symbol(&self, _symbol: &[String]) -> Option<String> {
    None
  }

  fn resolve_import_href(
    &self,
    _symbol: &[String],
    _src: &str,
  ) -> Option<String> {
    None
  }

  fn resolve_usage(&self, current_resolve: UrlResolveKind) -> Option<String> {
    current_resolve
      .get_file()
      .map(|file| format!("node:{}", file.path))
  }

  fn resolve_source(&self, _location: &deno_doc::Location) -> Option<String> {
    None
  }

  fn resolve_external_jsdoc_module(
    &self,
    _module: &str,
    _symbol: Option<&str>,
  ) -> Option<(String, String)> {
    None
  }
}

fn generate_docs_directory(
  doc_nodes_by_url: IndexMap<ModuleSpecifier, Vec<doc::DocNode>>,
  html_options: &DocHtmlFlag,
  deno_ns: std::collections::HashMap<Vec<String>, Option<Rc<ShortPath>>>,
  rewrite_map: Option<IndexMap<ModuleSpecifier, String>>,
) -> Result<(), AnyError> {
  let cwd = std::env::current_dir().context("Failed to get CWD")?;
  let output_dir_resolved = cwd.join(&html_options.output);

  let internal_env = std::env::var("DENO_INTERNAL_HTML_DOCS").ok();

  let href_resolver: Rc<dyn deno_doc::html::HrefResolver> = if internal_env
    .as_ref()
    .is_some_and(|internal_html_docs| internal_html_docs == "node")
  {
    Rc::new(NodeDocResolver(html_options.strip_trailing_html))
  } else if internal_env
    .as_ref()
    .is_some_and(|internal_html_docs| internal_html_docs == "deno")
    || deno_ns.is_empty()
  {
    Rc::new(DenoDocResolver(html_options.strip_trailing_html))
  } else {
    Rc::new(DocResolver {
      deno_ns,
      strip_trailing_html: html_options.strip_trailing_html,
    })
  };

  let category_docs =
    if let Some(category_docs_path) = &html_options.category_docs_path {
      let content = std::fs::read(category_docs_path)?;
      Some(deno_core::serde_json::from_slice(&content)?)
    } else {
      None
    };

  let symbol_redirect_map = if let Some(symbol_redirect_map_path) =
    &html_options.symbol_redirect_map_path
  {
    let content = std::fs::read(symbol_redirect_map_path)?;
    Some(deno_core::serde_json::from_slice(&content)?)
  } else {
    None
  };

  let default_symbol_map = if let Some(default_symbol_map_path) =
    &html_options.default_symbol_map_path
  {
    let content = std::fs::read(default_symbol_map_path)?;
    Some(deno_core::serde_json::from_slice(&content)?)
  } else {
    None
  };

  let options = deno_doc::html::GenerateOptions {
    package_name: html_options.name.clone(),
    main_entrypoint: None,
    rewrite_map,
    href_resolver,
    usage_composer: None,
    category_docs,
    disable_search: internal_env.is_some(),
    symbol_redirect_map,
    default_symbol_map,
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
  doc_nodes.retain(|doc_node| doc_node.kind() != doc::DocNodeKind::Import);
  let details = if let Some(filter) = doc_flags.filter {
    let nodes = doc::find_nodes_by_name_recursively(doc_nodes, &filter);
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

  display::write_to_stdout_ignore_sigpipe(details.as_bytes())
    .map_err(AnyError::from)
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

  for (_, diagnostics_by_lc) in diagnostic_groups {
    for (_, diagnostics_by_col) in diagnostics_by_lc {
      for (_, diagnostics) in diagnostics_by_col {
        for diagnostic in diagnostics {
          log::error!("{}\n", diagnostic.display());
        }
      }
    }
  }
  bail!(
    "Found {} documentation lint error{}.",
    colors::bold(diagnostics.len().to_string()),
    if diagnostics.len() == 1 { "" } else { "s" }
  );
}
