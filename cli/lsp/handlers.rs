// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::lsp_extensions;
use super::state::ServerStateSnapshot;
use super::text;

use crate::module_graph::Module;

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use dprint_plugin_typescript as dprint;
use lsp_types::DocumentFormattingParams;
use lsp_types::Hover;
use lsp_types::HoverParams;
use lsp_types::TextEdit;
use std::path::PathBuf;

pub fn handle_formatting(
  state: ServerStateSnapshot,
  params: DocumentFormattingParams,
) -> Result<Option<Vec<TextEdit>>, AnyError> {
  let specifier = ModuleSpecifier::from(params.text_document.uri.clone());
  let file_cache = state.file_cache.read().unwrap();
  let file_id = file_cache.lookup(&specifier).unwrap();
  let file_text = file_cache.get_contents(file_id)?;

  let file_path = if let Ok(file_path) = params.text_document.uri.to_file_path()
  {
    file_path
  } else {
    PathBuf::from(params.text_document.uri.path())
  };
  let config = dprint::configuration::ConfigurationBuilder::new()
    .deno()
    .build();

  let new_text = dprint::format_text(&file_path, &file_text, &config)
    .map_err(|e| custom_error("FormatError", e))?;

  let text_edits = text::get_edits(&file_text, &new_text);
  if text_edits.is_empty() {
    Ok(None)
  } else {
    Ok(Some(text_edits))
  }
}

pub fn handle_hover(
  state: ServerStateSnapshot,
  params: HoverParams,
) -> Result<Option<Hover>, AnyError> {
  let specifier = ModuleSpecifier::from(
    params.text_document_position_params.text_document.uri,
  );
  info!("hover specifier: {}", specifier);
  let mut file_cache = state.file_cache.write().unwrap();
  if let Some(cached_module) = file_cache.get_cached_module(specifier) {
    let mut module = Module::new(cached_module, true, None);
    module.parse()?;
    info!("deps: {:?}", module.dependencies);
  }

  Ok(None)
}

pub fn handle_virtual_text_document(
  state: ServerStateSnapshot,
  params: lsp_extensions::VirtualTextDocumentParams,
) -> Result<String, AnyError> {
  let specifier = ModuleSpecifier::from(params.text_document.uri);
  let url = specifier.as_url();
  let scheme = url.scheme();
  assert_eq!(
    scheme, "deno",
    "unexpected document scheme received: \"{}\"",
    scheme
  );
  let path = url.path();
  let contents = match path {
    "/status.md" => {
      let file_cache = state.file_cache.read().unwrap();
      format!(
        r#"# Deno Language Server Status

- Documents in memory: {}

"#,
        file_cache.len()
      )
    }
    _ => {
      info!("path: {}", path);
      "".to_string()
    }
  };
  Ok(contents)
}
