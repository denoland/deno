use std::collections::HashMap;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::serde_json;
use lspower::LanguageServer;
use lspower::lsp::ClientCapabilities;
use lspower::lsp::ClientInfo;
use lspower::lsp::DidChangeTextDocumentParams;
use lspower::lsp::DidOpenTextDocumentParams;
use lspower::lsp::InitializeParams;
use lspower::lsp::InitializedParams;
use lspower::lsp::Position;
use lspower::lsp::Range;
use lspower::lsp::TextDocumentContentChangeEvent;
use lspower::lsp::TextDocumentItem;
use lspower::lsp::VersionedTextDocumentIdentifier;

use super::client::Client;
use super::config::CompletionSettings;
use super::config::ImportCompletionSettings;
use super::config::WorkspaceSettings;

pub struct ReplLanguageServer {
  language_server: super::language_server::LanguageServer,
  document_specifier: ModuleSpecifier,
  document_version: i32,
  document_text: String,
  pending_text: String,
}

impl ReplLanguageServer {
  pub async fn new_initialized() -> Result<ReplLanguageServer, AnyError> {
    let language_server = super::language_server::LanguageServer::new(Client::new_for_repl());

    // todo(dsherret): handle if someone changes their directory via Deno.chdir
    let cwd = std::env::current_dir()?;
    let cwd_uri = ModuleSpecifier::from_directory_path(&cwd).map_err(|_| {
      anyhow!("Could not get URI from {}", cwd.display())
    })?;

    #[allow(deprecated)]
    language_server.initialize(InitializeParams {
      process_id: None,
      root_path: None,
      root_uri: Some(cwd_uri.clone()),
      initialization_options: Some(serde_json::to_value(get_repl_workspace_settings()).unwrap()),
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
    }).await?;

    language_server.initialized(InitializedParams {}).await;

    let document_version = 0;
    let document_specifier = cwd_uri.join("$deno$repl.ts").unwrap();
    let document_text = "".to_string();
    language_server.did_open(DidOpenTextDocumentParams {
      text_document: TextDocumentItem {
        uri: document_specifier.clone(),
        language_id: "typescript".to_string(),
        version: document_version,
        text: document_text.clone(),
      },
    }).await;

    Ok(ReplLanguageServer {
      language_server,
      document_specifier,
      document_version,
      document_text,
      pending_text: String::new(),
    })
  }

  pub async fn update_current_text(&mut self, new_text: &str) {
    let new_text = if new_text.ends_with('\n') { new_text.to_string() } else { format!("{}\n", new_text) };
    self.did_change(&new_text).await;
    self.pending_text = new_text;
  }

  pub fn commit_current_text(&mut self) {
    self.document_text.push_str(&self.pending_text);
    println!("Current Text: {:?}", self.document_text);
    self.pending_text = String::new();
  }

  async fn did_change(&mut self, new_text: &str) {
    self.document_version += 1;
    let current_line_count = self.document_text.chars().filter(|c| *c == '\n').count() as u32;
    let pending_line_count = self.pending_text.chars().filter(|c| *c == '\n').count() as u32;
    self.language_server.did_change(DidChangeTextDocumentParams {
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
    }).await;
  }
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
        // TODOODODODODOOO: Make sure that the "download" message is supressed in the repl. Example:
        // Download https://deno.land/.well-known/deno-import-intellisense.json
        hosts: HashMap::from([
          ("https://deno.land/x/".to_string(), true),
        ]),
      },
    },
  }
}
