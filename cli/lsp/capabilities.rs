// Copyright 2018-2025 the Deno authors. MIT license.

//!
//! Provides information about what capabilities that are supported by the
//! language server, which helps determine what messages are sent from the
//! client.
//!
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
    .map(|_| {
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
    .unwrap_or(CodeActionProviderCapability::Simple(true))
}

pub fn semantic_tokens_registration_options(
) -> SemanticTokensRegistrationOptions {
  const LANGUAGES: [&str; 4] = [
    "javascript",
    "javascriptreact",
    "typescript",
    "typescriptreact",
  ];
  const SCHEMES: [&str; 5] = [
    "file",
    "untitled",
    "deno",
    "vscode-notebook-cell",
    "deno-notebook-cell",
  ];
  let mut document_filters =
    Vec::with_capacity(LANGUAGES.len() * SCHEMES.len());
  for language in &LANGUAGES {
    for scheme in &SCHEMES {
      document_filters.push(DocumentFilter {
        language: Some(language.to_string()),
        scheme: Some(scheme.to_string()),
        pattern: None,
      });
    }
  }
  SemanticTokensRegistrationOptions {
    text_document_registration_options: TextDocumentRegistrationOptions {
      document_selector: Some(document_filters),
    },
    semantic_tokens_options: SemanticTokensOptions {
      legend: get_legend(),
      range: Some(true),
      full: Some(SemanticTokensFullOptions::Bool(true)),
      ..Default::default()
    },
    static_registration_options: Default::default(),
  }
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
      // Don't include "," here as it leads to confusing completion
      // behavior with function arguments. See https://github.com/denoland/deno/issues/20160
      all_commit_characters: Some(vec![
        ".".to_string(),
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
    execute_command_provider: Some(ExecuteCommandOptions {
      commands: vec![
        "deno.cache".to_string(),
        "deno.reloadImportRegistries".to_string(),
      ],
      ..Default::default()
    }),
    call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
    semantic_tokens_provider: if client_capabilities
      .text_document
      .as_ref()
      .and_then(|t| t.semantic_tokens.as_ref())
      .and_then(|s| s.dynamic_registration)
      .unwrap_or_default()
    {
      None
    } else {
      Some(
        SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
          semantic_tokens_registration_options(),
        ),
      )
    },
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
      "testingApi": true,
      "didRefreshDenoConfigurationTreeNotifications": true,
    })),
    inlay_hint_provider: Some(OneOf::Left(true)),
    position_encoding: None,
    diagnostic_provider: None,
    inline_value_provider: None,
    inline_completion_provider: None,
    notebook_document_sync: Some(OneOf::Left(NotebookDocumentSyncOptions {
      notebook_selector: vec![NotebookSelector::ByCells {
        notebook: None,
        cells: vec![
          NotebookCellSelector {
            language: "javascript".to_string(),
          },
          NotebookCellSelector {
            language: "javascriptreact".to_string(),
          },
          NotebookCellSelector {
            language: "typescript".to_string(),
          },
          NotebookCellSelector {
            language: "typescriptreact".to_string(),
          },
        ],
      }],
      save: Some(true),
    })),
  }
}
