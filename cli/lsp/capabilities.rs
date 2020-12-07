// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

///!
///! Provides information about what capabilities that are supported by the
///! language server, which helps determine what messages are sent from the
///! client.
///!
use lsp_types::ClientCapabilities;
use lsp_types::HoverProviderCapability;
use lsp_types::OneOf;
use lsp_types::SaveOptions;
use lsp_types::ServerCapabilities;
use lsp_types::TextDocumentSyncCapability;
use lsp_types::TextDocumentSyncKind;
use lsp_types::TextDocumentSyncOptions;

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
    completion_provider: None,
    signature_help_provider: None,
    declaration_provider: None,
    definition_provider: Some(OneOf::Left(true)),
    type_definition_provider: None,
    implementation_provider: None,
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
    semantic_highlighting: None,
    folding_range_provider: None,
    rename_provider: None,
    document_link_provider: None,
    color_provider: None,
    execute_command_provider: None,
    workspace: None,
    call_hierarchy_provider: None,
    semantic_tokens_provider: None,
    on_type_rename_provider: None,
    experimental: None,
  }
}
