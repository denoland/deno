// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

///!
///! Provides information about what capabilities that are supported by the
///! language server, which helps determine what messages are sent from the
///! client.
///!
use lspower::lsp::ClientCapabilities;
use lspower::lsp::CompletionOptions;
use lspower::lsp::HoverProviderCapability;
use lspower::lsp::ImplementationProviderCapability;
use lspower::lsp::OneOf;
use lspower::lsp::SaveOptions;
use lspower::lsp::ServerCapabilities;
use lspower::lsp::TextDocumentSyncCapability;
use lspower::lsp::TextDocumentSyncKind;
use lspower::lsp::TextDocumentSyncOptions;
use lspower::lsp::WorkDoneProgressOptions;

pub fn server_capabilities(
  _client_capabilities: &ClientCapabilities,
) -> ServerCapabilities {
  ServerCapabilities {
    text_document_sync: Some(TextDocumentSyncCapability::Options(
      TextDocumentSyncOptions {
        open_close: Some(true),
        change: Some(TextDocumentSyncKind::Incremental),
        will_save: None,
        will_save_wait_until: None,
        save: Some(SaveOptions::default().into()),
      },
    )),
    hover_provider: Some(HoverProviderCapability::Simple(true)),
    completion_provider: Some(CompletionOptions {
      trigger_characters: Some(vec![
        ".".to_string(),
        "\"".to_string(),
        "'".to_string(),
        "`".to_string(),
        "/".to_string(),
        "@".to_string(),
        "<".to_string(),
        "#".to_string(),
      ]),
      resolve_provider: None,
      work_done_progress_options: WorkDoneProgressOptions {
        work_done_progress: None,
      },
    }),
    signature_help_provider: None,
    declaration_provider: None,
    definition_provider: Some(OneOf::Left(true)),
    type_definition_provider: None,
    implementation_provider: Some(ImplementationProviderCapability::Simple(
      true,
    )),
    references_provider: Some(OneOf::Left(true)),
    document_highlight_provider: Some(OneOf::Left(true)),
    document_symbol_provider: None,
    workspace_symbol_provider: None,
    code_action_provider: None,
    code_lens_provider: None,
    document_formatting_provider: Some(OneOf::Left(true)),
    document_range_formatting_provider: None,
    document_on_type_formatting_provider: None,
    selection_range_provider: None,
    folding_range_provider: None,
    rename_provider: Some(OneOf::Left(true)),
    document_link_provider: None,
    color_provider: None,
    execute_command_provider: None,
    call_hierarchy_provider: None,
    semantic_tokens_provider: None,
    workspace: None,
    experimental: None,
    linked_editing_range_provider: None,
    moniker_provider: None,
  }
}
