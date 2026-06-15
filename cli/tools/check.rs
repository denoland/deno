// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::swc::ast;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_terminal::colors;

use crate::args::CheckFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::graph_container::CheckSpecifiersOptions;
use crate::graph_container::CollectSpecifiersOptions;
use crate::graph_util::CliJsrUrlProvider;
use crate::util::extract;
use crate::util::file_watcher;
use crate::util::fs::specifier_from_file_path;

pub async fn check(
  flags: Arc<Flags>,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  if let Some(watch_flags) = &check_flags.watch {
    let no_clear_screen = watch_flags.no_clear_screen;
    file_watcher::watch_func(
      flags,
      file_watcher::PrintConfig::new("Check", !no_clear_screen),
      move |flags, watcher_communicator, changed_paths| {
        let check_flags = check_flags.clone();
        watcher_communicator.show_path_changed(changed_paths);
        Ok(async move {
          let factory = CliFactory::from_flags_for_watcher(
            flags,
            watcher_communicator.clone(),
          );
          check_with_factory(&factory, check_flags).await
        })
      },
    )
    .await
  } else {
    let factory = CliFactory::from_flags(flags);
    check_with_factory(&factory, check_flags).await
  }
}

async fn check_with_factory(
  factory: &CliFactory,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  let main_graph_container = factory.main_module_graph_container().await?;

  let specifiers = main_graph_container.collect_specifiers(
    &check_flags.files,
    CollectSpecifiersOptions {
      include_ignored_specified: false,
    },
  )?;
  let ambient_declaration_specifiers =
    if check_flags.doc || check_flags.doc_only {
      Vec::new()
    } else {
      collect_ambient_declaration_specifiers(
        factory,
        &check_flags.files,
        &specifiers,
      )?
    };
  if specifiers.is_empty() {
    log::warn!("{} No matching files found.", colors::yellow("Warning"));
  }

  let specifiers_for_typecheck = if check_flags.doc || check_flags.doc_only {
    let file_fetcher = factory.file_fetcher()?;
    let root_permissions = factory.root_permissions_container()?;

    let mut specifiers_for_typecheck = if check_flags.doc {
      specifiers.clone()
    } else {
      vec![]
    };

    for s in specifiers {
      let file = file_fetcher.fetch(&s, root_permissions).await?;
      let snippet_files = extract::extract_snippet_files(file)?;
      for snippet_file in snippet_files {
        specifiers_for_typecheck.push(snippet_file.url.clone());
        file_fetcher.insert_memory_files(snippet_file);
      }
    }

    specifiers_for_typecheck
  } else {
    specifiers
  };

  main_graph_container
    .check_specifiers(
      &specifiers_for_typecheck,
      CheckSpecifiersOptions {
        allow_unknown_media_types: true,
        extra_imports: ambient_declaration_imports(
          &ambient_declaration_specifiers,
          &specifiers_for_typecheck,
        ),
        extra_type_roots: ambient_declaration_specifiers,
        ..Default::default()
      },
    )
    .await
}

fn ambient_declaration_imports(
  declarations: &[Url],
  roots: &[Url],
) -> Vec<deno_graph::ReferrerImports> {
  let Some(referrer) = roots.first() else {
    return Vec::new();
  };
  if declarations.is_empty() {
    return Vec::new();
  }
  vec![deno_graph::ReferrerImports {
    referrer: referrer.clone(),
    imports: declarations.iter().map(ToString::to_string).collect(),
  }]
}

fn collect_ambient_declaration_specifiers(
  factory: &CliFactory,
  files: &[String],
  collected_specifiers: &[Url],
) -> Result<Vec<Url>, AnyError> {
  let cli_options = factory.cli_options()?;
  let excludes = cli_options.workspace().resolve_config_excludes()?;
  let vendor_dir = cli_options.vendor_dir_path().map(|p| p.as_path());
  let collected_specifiers =
    collected_specifiers.iter().cloned().collect::<HashSet<_>>();
  let mut dirs = HashSet::<PathBuf>::new();
  let mut ambient_module_names = HashSet::new();

  for file in files {
    if Url::parse(file)
      .is_ok_and(|url| url.scheme() != "file" && url.scheme().len() != 1)
    {
      continue;
    }
    let Ok(specifier) =
      deno_path_util::resolve_path(file, cli_options.initial_cwd())
    else {
      continue;
    };
    if !collected_specifiers.contains(&specifier) {
      continue;
    }
    let Ok(path) = deno_path_util::url_to_file_path(&specifier) else {
      continue;
    };
    let Ok(metadata) = std::fs::metadata(&path) else {
      continue;
    };
    if !metadata.is_file() {
      continue;
    }
    collect_bare_import_specifiers(
      &mut ambient_module_names,
      &specifier,
      &path,
    );
    if let Some(parent) = path.parent() {
      dirs.insert(parent.to_path_buf());
    }
  }

  if ambient_module_names.is_empty() {
    return Ok(Vec::new());
  }

  let mut specifiers = Vec::new();
  let mut seen = collected_specifiers;
  let mut dirs = dirs.into_iter().collect::<Vec<_>>();
  dirs.sort();
  for dir in dirs {
    let Ok(entries) = std::fs::read_dir(dir) else {
      continue;
    };
    for entry in entries.flatten() {
      let path = entry.path();
      if !path.is_file()
        || !MediaType::from_path(&path).is_declaration()
        || excludes.matches_path(&path)
        || is_ignored_declaration_path(&path, vendor_dir)
        || !has_exact_ambient_module_declaration(&path, &ambient_module_names)
      {
        continue;
      }
      let specifier = specifier_from_file_path(&path)?;
      if seen.insert(specifier.clone()) {
        specifiers.push(specifier);
      }
    }
  }
  specifiers.sort();
  Ok(specifiers)
}

fn collect_bare_import_specifiers(
  ambient_module_names: &mut HashSet<String>,
  specifier: &Url,
  path: &Path,
) {
  let media_type = MediaType::from_path(path);
  if media_type.is_declaration() {
    return;
  }
  let Ok(source) = std::fs::read_to_string(path) else {
    return;
  };
  let Ok(parsed_source) = deno_ast::parse_module(deno_ast::ParseParams {
    specifier: specifier.clone(),
    text: source.into(),
    media_type,
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  }) else {
    return;
  };
  let module =
    deno_graph::parse_module_from_ast(deno_graph::ParseModuleFromAstOptions {
      graph_kind: deno_graph::GraphKind::All,
      specifier: specifier.clone(),
      maybe_headers: None,
      mtime: None,
      parsed_source: &parsed_source,
      file_system: &deno_graph::source::NullFileSystem,
      jsr_url_provider: &CliJsrUrlProvider,
      maybe_resolver: None,
    });
  for specifier_text in module.dependencies.keys() {
    if !deno_path_util::is_relative_specifier(specifier_text)
      && Url::parse(specifier_text).is_err()
    {
      ambient_module_names.insert(specifier_text.to_string());
    }
  }
}

fn is_ignored_declaration_path(path: &Path, vendor_dir: Option<&Path>) -> bool {
  path.components().any(|component| {
    let component = component.as_os_str();
    component == "node_modules" || component == ".git"
  }) || vendor_dir.is_some_and(|vendor_dir| path.starts_with(vendor_dir))
}

fn has_exact_ambient_module_declaration(
  path: &Path,
  ambient_module_names: &HashSet<String>,
) -> bool {
  struct AmbientModuleVisitor<'a> {
    ambient_module_names: &'a HashSet<String>,
    found: bool,
  }

  impl Visit for AmbientModuleVisitor<'_> {
    fn visit_ts_module_decl(&mut self, ts_module_decl: &ast::TsModuleDecl) {
      if self.found {
        return;
      }
      if ts_module_decl.declare
        && let ast::TsModuleName::Str(module_name) = &ts_module_decl.id
        && let Some(module_name) = module_name.value.as_str()
        && self.ambient_module_names.contains(module_name)
      {
        self.found = true;
        return;
      }
      ts_module_decl.visit_children_with(self);
    }
  }

  let Ok(source) = std::fs::read_to_string(path) else {
    return false;
  };
  let Ok(specifier) = specifier_from_file_path(path) else {
    return false;
  };
  let Ok(parsed_source) = deno_ast::parse_module(deno_ast::ParseParams {
    specifier,
    text: source.into(),
    media_type: MediaType::from_path(path),
    capture_tokens: false,
    scope_analysis: false,
    maybe_syntax: None,
  }) else {
    return false;
  };
  let mut visitor = AmbientModuleVisitor {
    ambient_module_names,
    found: false,
  };
  parsed_source.program().visit_with(&mut visitor);
  visitor.found
}
