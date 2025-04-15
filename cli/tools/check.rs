// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
use deno_terminal::colors;

use crate::args::CheckFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::util::extract;

pub async fn check(
  flags: Arc<Flags>,
  check_flags: CheckFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags);

  let main_graph_container = factory.main_module_graph_container().await?;

  let specifiers =
    main_graph_container.collect_specifiers(&check_flags.files)?;
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
    .check_specifiers(&specifiers_for_typecheck, Default::default())
    .await
}
