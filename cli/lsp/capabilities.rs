// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

///!
///! Provides information about what capabilities that are supported by the
///! language server, which helps determine what messages are sent from the
///! client.
///!
use deno_core::serde_json::json;
use tower_lsp::lsp_types::*;

use super::refactor::ALL_KNOWN_REFACTOR_ACTION_KINDS;
use super::semantic_tokens::get_legend;

fn code_action_capabilities(
  client_capabilities: &ClientCapabilities,
) -> CodeActionProviderCapability {
  client_capabilities
    .text_document
    .as_ref()
    .and_then(|it| it.code_action.as_ref())
    .and_then(|it| it.code_action_literal_support.as_ref())
    .map_or(CodeActionProviderCapability::Simple(true), |_| {
      let mut code_action_kinds =
        vec![CodeActionKind::QUICKFIX, CodeActionKind::REFACTOR];
      code_action_kinds.extend(
        ALL_KNOWN_REFACTOR_ACTION_KINDS
          .iter()
          .map(|action| action.kind.clone()),
      );

      CodeActionProviderCapability::Options(CodeActionOptions {
        code_action_kinds: Some(code_action_kinds),
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
        change: Some(TextDocumentSyncKind::INCREMENTAL),
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
      completion_item: None,
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
    type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(
      true,
    )),
    implementation_provider: Some(ImplementationProviderCapability::Simple(
      true,
    )),
    references_provider: Some(OneOf::Left(true)),
    document_highlight_provider: Some(OneOf::Left(true)),
    document_symbol_provider: Some(OneOf::Right(DocumentSymbolOptions {
      label: Some("Deno".to_string()),
      work_done_progress_options: WorkDoneProgressOptions {
        work_done_progress: None,
      },
    })),
    workspace_symbol_provider: Some(OneOf::Left(true)),
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
    semantic_tokens_provider: Some(
      SemanticTokensServerCapabilities::SemanticTokensOptions(
        SemanticTokensOptions {
          legend: get_legend(),
          range: Some(true),
          full: Some(SemanticTokensFullOptions::Bool(true)),
          ..Default::default()
        },
      ),
    ),
    workspace: Some(WorkspaceServerCapabilities {
      workspace_folders: Some(WorkspaceFoldersServerCapabilities {
        supported: Some(true),
        change_notifications: Some(OneOf::Left(true)),
      }),
      file_operations: None,
    }),
    linked_editing_range_provider: None,
    moniker_provider: None,
    experimental: Some(json!({
      "denoConfigTasks": true,
      "testingApi":true,
    })),
    inlay_hint_provider: Some(OneOf::Left(true)),
  }
}
