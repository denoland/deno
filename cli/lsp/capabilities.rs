// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

///!
///! Provides information about what capabilities that are supported by the
///! language server, which helps determine what messages are sent from the
///! client.
///!
use lspower::lsp::CallHierarchyServerCapability;
use lspower::lsp::ClientCapabilities;
use lspower::lsp::CodeActionKind;
use lspower::lsp::CodeActionOptions;
use lspower::lsp::CodeActionProviderCapability;
use lspower::lsp::CodeLensOptions;
use lspower::lsp::CompletionOptions;
use lspower::lsp::FoldingRangeProviderCapability;
use lspower::lsp::HoverProviderCapability;
use lspower::lsp::ImplementationProviderCapability;
use lspower::lsp::OneOf;
use lspower::lsp::SaveOptions;
use lspower::lsp::SelectionRangeProviderCapability;
use lspower::lsp::ServerCapabilities;
use lspower::lsp::SignatureHelpOptions;
use lspower::lsp::TextDocumentSyncCapability;
use lspower::lsp::TextDocumentSyncKind;
use lspower::lsp::TextDocumentSyncOptions;
use lspower::lsp::WorkDoneProgressOptions;

fn code_action_capabilities(
  client_capabilities: &ClientCapabilities,
) -> CodeActionProviderCapability {
  client_capabilities
    .text_document
    .as_ref()
    .and_then(|it| it.code_action.as_ref())
    .and_then(|it| it.code_action_literal_support.as_ref())
    .map_or(CodeActionProviderCapability::Simple(true), |_| {
      CodeActionProviderCapability::Options(CodeActionOptions {
        code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
        resolve_provider: Some(true),
        work_done_progress_options: Default::default(),
      })
    })
}

pub fn server_capabilities(
  client_capabilities: &ClientCapabilities,
) -> ServerCapabilities {
  let code_action_provider = code_action_capabilities(client_capabilities);
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
      all_commit_characters: Some(vec![
        ".".to_string(),
        ",".to_string(),
        ";".to_string(),
        "(".to_string(),
      ]),
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
      resolve_provider: Some(true),
      work_done_progress_options: WorkDoneProgressOptions {
        work_done_progress: None,
      },
    }),
    signature_help_provider: Some(SignatureHelpOptions {
      trigger_characters: Some(vec![
        ",".to_string(),
        "(".to_string(),
        "<".to_string(),
      ]),
      retrigger_characters: Some(vec![")".to_string()]),
      work_done_progress_options: WorkDoneProgressOptions {
        work_done_progress: None,
      },
    }),
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
    code_action_provider: Some(code_action_provider),
    code_lens_provider: Some(CodeLensOptions {
      resolve_provider: Some(true),
    }),
    document_formatting_provider: Some(OneOf::Left(true)),
    document_range_formatting_provider: None,
    document_on_type_formatting_provider: None,
    selection_range_provider: Some(SelectionRangeProviderCapability::Simple(
      true,
    )),
    folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
    rename_provider: Some(OneOf::Left(true)),
    document_link_provider: None,
    color_provider: None,
    execute_command_provider: None,
    call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
    semantic_tokens_provider: None,
    workspace: None,
    experimental: None,
    linked_editing_range_provider: None,
    moniker_provider: None,
  }
}
