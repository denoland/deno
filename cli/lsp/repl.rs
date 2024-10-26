// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;

use deno_ast::LineAndColumnIndex;
use deno_ast::ModuleSpecifier;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use lsp_types::Uri;
use tower_lsp::lsp_types::ClientCapabilities;
use tower_lsp::lsp_types::ClientInfo;
use tower_lsp::lsp_types::CompletionContext;
use tower_lsp::lsp_types::CompletionParams;
use tower_lsp::lsp_types::CompletionResponse;
use tower_lsp::lsp_types::CompletionTextEdit;
use tower_lsp::lsp_types::CompletionTriggerKind;
use tower_lsp::lsp_types::DidChangeTextDocumentParams;
use tower_lsp::lsp_types::DidCloseTextDocumentParams;
use tower_lsp::lsp_types::DidOpenTextDocumentParams;
use tower_lsp::lsp_types::InitializeParams;
use tower_lsp::lsp_types::InitializedParams;
use tower_lsp::lsp_types::PartialResultParams;
use tower_lsp::lsp_types::Position;
use tower_lsp::lsp_types::Range;
use tower_lsp::lsp_types::TextDocumentContentChangeEvent;
use tower_lsp::lsp_types::TextDocumentIdentifier;
use tower_lsp::lsp_types::TextDocumentItem;
use tower_lsp::lsp_types::TextDocumentPositionParams;
use tower_lsp::lsp_types::VersionedTextDocumentIdentifier;
use tower_lsp::lsp_types::WorkDoneProgressParams;
use tower_lsp::LanguageServer;

use super::client::Client;
use super::config::ClassMemberSnippets;
use super::config::CompletionSettings;
use super::config::DenoCompletionSettings;
use super::config::ImportCompletionSettings;
use super::config::LanguageWorkspaceSettings;
use super::config::ObjectLiteralMethodSnippets;
use super::config::TestingSettings;
use super::config::WorkspaceSettings;
use super::urls::uri_parse_unencoded;
use super::urls::url_to_uri;

#[derive(Debug)]
pub struct ReplCompletionItem {
  pub new_text: String,
  pub range: std::ops::Range<usize>,
}

pub struct ReplLanguageServer {
  language_server: super::language_server::LanguageServer,
  document_version: i32,
  document_text: String,
  pending_text: String,
  cwd_uri: ModuleSpecifier,
}

impl ReplLanguageServer {
  pub async fn new_initialized() -> Result<ReplLanguageServer, AnyError> {
    // downgrade info and warn lsp logging to debug
    super::logging::set_lsp_log_level(log::Level::Debug);
    super::logging::set_lsp_warn_level(log::Level::Debug);

    let language_server = super::language_server::LanguageServer::new(
      Client::new_for_repl(),
      Default::default(),
    );

    let cwd_uri = get_cwd_uri()?;

    #[allow(deprecated)]
    language_server
      .initialize(InitializeParams {
        process_id: None,
        root_path: None,
        root_uri: Some(url_to_uri(&cwd_uri).unwrap()),
        initialization_options: Some(
          serde_json::to_value(get_repl_workspace_settings()).unwrap(),
        ),
        capabilities: ClientCapabilities {
          workspace: None,
          text_document: None,
          window: None,
          general: None,
          experimental: None,
          offset_encoding: None,
          notebook_document: None,
        },
        trace: None,
        workspace_folders: None,
        client_info: Some(ClientInfo {
          name: "Deno REPL".to_string(),
          version: None,
        }),
        locale: None,
        work_done_progress_params: Default::default(),
      })
      .await?;

    language_server.initialized(InitializedParams {}).await;

    let server = ReplLanguageServer {
      language_server,
      document_version: 0,
      document_text: String::new(),
      pending_text: String::new(),
      cwd_uri,
    };
    server.open_current_document().await;

    Ok(server)
  }

  pub async fn commit_text(&mut self, line_text: &str) {
    self.did_change(line_text).await;
    self.document_text.push_str(&self.pending_text);
    self.pending_text = String::new();
  }

  pub async fn completions(
    &mut self,
    line_text: &str,
    position: usize,
  ) -> Vec<ReplCompletionItem> {
    self.did_change(line_text).await;
    let text_info = deno_ast::SourceTextInfo::from_string(format!(
      "{}{}",
      self.document_text, self.pending_text
    ));
    let before_line_len = self.document_text.len();
    let position = text_info.range().start + before_line_len + position;
    let line_and_column = text_info.line_and_column_index(position);
    let response = self
      .language_server
      .completion(CompletionParams {
        text_document_position: TextDocumentPositionParams {
          text_document: TextDocumentIdentifier {
            uri: self.get_document_uri(),
          },
          position: Position {
            line: line_and_column.line_index as u32,
            character: line_and_column.column_index as u32,
          },
        },
        work_done_progress_params: WorkDoneProgressParams {
          work_done_token: None,
        },
        partial_result_params: PartialResultParams {
          partial_result_token: None,
        },
        context: Some(CompletionContext {
          trigger_kind: CompletionTriggerKind::INVOKED,
          trigger_character: None,
        }),
      })
      .await
      .ok()
      .unwrap_or_default();

    let mut items = match response {
      Some(CompletionResponse::Array(items)) => items,
      Some(CompletionResponse::List(list)) => list.items,
      None => Vec::new(),
    };
    items.sort_by_key(|item| {
      if let Some(sort_text) = &item.sort_text {
        sort_text.clone()
      } else {
        item.label.clone()
      }
    });
    items
      .into_iter()
      .filter_map(|item| {
        item.text_edit.and_then(|edit| match edit {
          CompletionTextEdit::Edit(edit) => Some(ReplCompletionItem {
            new_text: edit.new_text,
            range: lsp_range_to_std_range(&text_info, &edit.range),
          }),
          CompletionTextEdit::InsertAndReplace(_) => None,
        })
      })
      .filter(|item| {
        // filter the results to only exact matches
        let text = &text_info.text_str()[item.range.clone()];
        item.new_text.starts_with(text)
      })
      .map(|mut item| {
        // convert back to a line position
        item.range.start -= before_line_len;
        item.range.end -= before_line_len;
        item
      })
      .collect()
  }

  async fn did_change(&mut self, new_text: &str) {
    self.check_cwd_change().await;
    let new_text = if new_text.ends_with('\n') {
      new_text.to_string()
    } else {
      format!("{new_text}\n")
    };
    self.document_version += 1;
    let current_line_count =
      self.document_text.chars().filter(|c| *c == '\n').count() as u32;
    let pending_line_count =
      self.pending_text.chars().filter(|c| *c == '\n').count() as u32;
    self
      .language_server
      .did_change(DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
          uri: self.get_document_uri(),
          version: self.document_version,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
          range: Some(Range {
            start: Position::new(current_line_count, 0),
            end: Position::new(current_line_count + pending_line_count, 0),
          }),
          range_length: None,
          text: new_text.to_string(),
        }],
      })
      .await;
    self.pending_text = new_text;
  }

  async fn check_cwd_change(&mut self) {
    // handle if the cwd changes, if the cwd is deleted in the case of
    // get_cwd_uri() erroring, then keep using it as the base
    let cwd_uri = get_cwd_uri().unwrap_or_else(|_| self.cwd_uri.clone());
    if self.cwd_uri != cwd_uri {
      self
        .language_server
        .did_close(DidCloseTextDocumentParams {
          text_document: TextDocumentIdentifier {
            uri: self.get_document_uri(),
          },
        })
        .await;
      self.cwd_uri = cwd_uri;
      self.document_version = 0;
      self.open_current_document().await;
    }
  }

  async fn open_current_document(&self) {
    self
      .language_server
      .did_open(DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
          uri: self.get_document_uri(),
          language_id: "typescript".to_string(),
          version: self.document_version,
          text: format!("{}{}", self.document_text, self.pending_text),
        },
      })
      .await;
  }

  fn get_document_uri(&self) -> Uri {
    uri_parse_unencoded(self.cwd_uri.join("$deno$repl.ts").unwrap().as_str())
      .unwrap()
  }
}

fn lsp_range_to_std_range(
  text_info: &SourceTextInfo,
  range: &Range,
) -> std::ops::Range<usize> {
  let start_index = text_info
    .loc_to_source_pos(LineAndColumnIndex {
      line_index: range.start.line as usize,
      column_index: range.start.character as usize,
    })
    .as_byte_index(text_info.range().start);
  let end_index = text_info
    .loc_to_source_pos(LineAndColumnIndex {
      line_index: range.end.line as usize,
      column_index: range.end.character as usize,
    })
    .as_byte_index(text_info.range().start);

  start_index..end_index
}

fn get_cwd_uri() -> Result<ModuleSpecifier, AnyError> {
  let cwd = std::env::current_dir()?;
  ModuleSpecifier::from_directory_path(&cwd)
    .map_err(|_| anyhow!("Could not get URI from {}", cwd.display()))
}

pub fn get_repl_workspace_settings() -> WorkspaceSettings {
  WorkspaceSettings {
    enable: Some(true),
    disable_paths: vec![],
    enable_paths: None,
    config: None,
    certificate_stores: None,
    cache: None,
    cache_on_save: false,
    import_map: None,
    code_lens: Default::default(),
    internal_debug: false,
    internal_inspect: Default::default(),
    log_file: false,
    lint: false,
    document_preload_limit: 0, // don't pre-load any modules as it's expensive and not useful for the repl
    tls_certificate: None,
    unsafely_ignore_certificate_errors: None,
    unstable: Default::default(),
    suggest: DenoCompletionSettings {
      imports: ImportCompletionSettings {
        auto_discover: false,
        hosts: HashMap::from([("https://deno.land".to_string(), true)]),
      },
    },
    testing: TestingSettings { args: vec![] },
    javascript: LanguageWorkspaceSettings {
      suggest: CompletionSettings {
        auto_imports: false,
        class_member_snippets: ClassMemberSnippets { enabled: false },
        complete_function_calls: false,
        enabled: true,
        include_automatic_optional_chain_completions: false,
        include_completions_for_import_statements: true,
        names: false,
        object_literal_method_snippets: ObjectLiteralMethodSnippets {
          enabled: false,
        },
        paths: false,
      },
      ..Default::default()
    },
    typescript: LanguageWorkspaceSettings {
      suggest: CompletionSettings {
        auto_imports: false,
        class_member_snippets: ClassMemberSnippets { enabled: false },
        complete_function_calls: false,
        enabled: true,
        include_automatic_optional_chain_completions: false,
        include_completions_for_import_statements: true,
        names: false,
        object_literal_method_snippets: ObjectLiteralMethodSnippets {
          enabled: false,
        },
        paths: false,
      },
      ..Default::default()
    },
  }
}
