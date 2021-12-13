use std::collections::HashMap;
use std::future::Future;

use deno_ast::swc::common::BytePos;
use deno_ast::swc::common::Span;
use deno_ast::LineAndColumnIndex;
use deno_ast::ModuleSpecifier;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use lspower::lsp::ClientCapabilities;
use lspower::lsp::ClientInfo;
use lspower::lsp::CompletionParams;
use lspower::lsp::CompletionResponse;
use lspower::lsp::CompletionTextEdit;
use lspower::lsp::DidChangeTextDocumentParams;
use lspower::lsp::DidOpenTextDocumentParams;
use lspower::lsp::InitializeParams;
use lspower::lsp::InitializedParams;
use lspower::lsp::PartialResultParams;
use lspower::lsp::Position;
use lspower::lsp::Range;
use lspower::lsp::TextDocumentContentChangeEvent;
use lspower::lsp::TextDocumentIdentifier;
use lspower::lsp::TextDocumentItem;
use lspower::lsp::TextDocumentPositionParams;
use lspower::lsp::VersionedTextDocumentIdentifier;
use lspower::lsp::WorkDoneProgressParams;
use lspower::LanguageServer;

use crate::logger;

use super::client::Client;
use super::config::CompletionSettings;
use super::config::ImportCompletionSettings;
use super::config::WorkspaceSettings;

#[derive(Debug)]
pub struct ReplCompletionItem {
  pub new_text: String,
  pub span: Span,
}

pub struct ReplLanguageServer {
  language_server: super::language_server::LanguageServer,
  document_specifier: ModuleSpecifier,
  document_version: i32,
  document_text: String,
  pending_text: String,
}

impl ReplLanguageServer {
  pub async fn new_initialized() -> Result<ReplLanguageServer, AnyError> {
    with_logging_disabled(async {
      let language_server =
        super::language_server::LanguageServer::new(Client::new_for_repl());

      // todo(dsherret): handle if someone changes their directory via Deno.chdir
      let cwd = std::env::current_dir()?;
      let cwd_uri = ModuleSpecifier::from_directory_path(&cwd)
        .map_err(|_| anyhow!("Could not get URI from {}", cwd.display()))?;

      #[allow(deprecated)]
      language_server
        .initialize(InitializeParams {
          process_id: None,
          root_path: None,
          root_uri: Some(cwd_uri.clone()),
          initialization_options: Some(
            serde_json::to_value(get_repl_workspace_settings()).unwrap(),
          ),
          capabilities: ClientCapabilities {
            workspace: None,
            text_document: None,
            window: None,
            general: None,
            //offset_encoding: None,
            experimental: None,
          },
          trace: None,
          workspace_folders: None,
          client_info: Some(ClientInfo {
            name: "Deno REPL".to_string(),
            version: None,
          }),
          locale: None,
        })
        .await?;

      language_server.initialized(InitializedParams {}).await;

      let document_version = 0;
      let document_specifier = cwd_uri.join("$deno$repl.ts").unwrap();
      let document_text = "".to_string();
      language_server
        .did_open(DidOpenTextDocumentParams {
          text_document: TextDocumentItem {
            uri: document_specifier.clone(),
            language_id: "typescript".to_string(),
            version: document_version,
            text: document_text.clone(),
          },
        })
        .await;

      Ok(ReplLanguageServer {
        language_server,
        document_specifier,
        document_version,
        document_text,
        pending_text: String::new(),
      })
    })
    .await
  }

  pub async fn commit_text(&mut self, line_text: &str) {
    with_logging_disabled(async {
      self.did_change(line_text).await;
      self.document_text.push_str(&self.pending_text);
      self.pending_text = String::new();
    })
    .await
  }

  pub async fn completions(
    &mut self,
    line_text: &str,
    position: usize,
  ) -> Vec<ReplCompletionItem> {
    with_logging_disabled(async {
      self.did_change(line_text).await;
      let before_line_len = BytePos(self.document_text.len() as u32);
      let position = before_line_len + BytePos(position as u32);
      let text_info = deno_ast::SourceTextInfo::from_string(format!(
        "{}{}",
        self.document_text, self.pending_text
      ));
      let line_and_column = text_info.line_and_column_index(position);
      let response = self
        .language_server
        .completion(CompletionParams {
          text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
              uri: self.document_specifier.clone(),
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
          context: None,
        })
        .await
        .ok()
        .unwrap_or_default();

      let items = match response {
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        None => Vec::new(),
      };
      items
        .into_iter()
        .filter_map(|item| {
          item.text_edit.and_then(|edit| match edit {
            CompletionTextEdit::Edit(edit) => Some(ReplCompletionItem {
              new_text: edit.new_text,
              span: lsp_range_to_span(&text_info, &edit.range),
            }),
            CompletionTextEdit::InsertAndReplace(_) => None,
          })
        })
        .filter(|item| {
          // filter the results to only exact matches
          let text = &text_info.text_str()
            [item.span.lo.0 as usize..item.span.hi.0 as usize];
          item.new_text.starts_with(text)
        })
        .map(|mut item| {
          // convert back to a line position
          item.span = Span::new(
            item.span.lo - before_line_len,
            item.span.hi - before_line_len,
            Default::default(),
          );
          item
        })
        .collect()
    })
    .await
  }

  async fn did_change(&mut self, new_text: &str) {
    let new_text = if new_text.ends_with('\n') {
      new_text.to_string()
    } else {
      format!("{}\n", new_text)
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
          uri: self.document_specifier.clone(),
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
}

async fn with_logging_disabled<TReturn>(
  future: impl Future<Output = TReturn>,
) -> TReturn {
  logger::set_logging_enabled(false);
  let result = future.await;
  // ensure this is set back even on error of a Result
  logger::set_logging_enabled(true);
  result
}

pub fn get_repl_workspace_settings() -> WorkspaceSettings {
  WorkspaceSettings {
    enable: true,
    config: None,
    cache: None,
    import_map: None,
    code_lens: Default::default(),
    internal_debug: false,
    lint: false,
    unstable: false,
    suggest: CompletionSettings {
      complete_function_calls: false,
      names: false,
      paths: false,
      auto_imports: false,
      imports: ImportCompletionSettings {
        auto_discover: true,
        hosts: HashMap::from([("https://deno.land/x/".to_string(), true)]),
      },
    },
  }
}

fn lsp_range_to_span(text_info: &SourceTextInfo, range: &Range) -> Span {
  Span::new(
    text_info.byte_index(LineAndColumnIndex {
      line_index: range.start.line as usize,
      column_index: range.start.character as usize,
    }),
    text_info.byte_index(LineAndColumnIndex {
      line_index: range.end.line as usize,
      column_index: range.end.character as usize,
    }),
    Default::default(),
  )
}
