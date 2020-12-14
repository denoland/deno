// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

mod analysis;
mod capabilities;
mod config;
mod diagnostics;
mod dispatch;
mod handlers;
mod lsp_extensions;
mod memory_cache;
mod sources;
mod state;
mod text;
mod tsc;
mod utils;

use config::Config;
use diagnostics::DiagnosticSource;
use dispatch::NotificationDispatcher;
use dispatch::RequestDispatcher;
use state::update_import_map;
use state::DocumentData;
use state::Event;
use state::ServerState;
use state::Status;
use state::Task;
use text::apply_content_changes;

use crate::tsc_config::TsConfig;

use crossbeam_channel::Receiver;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use lsp_server::Connection;
use lsp_server::ErrorCode;
use lsp_server::Message;
use lsp_server::Notification;
use lsp_server::Request;
use lsp_server::RequestId;
use lsp_server::Response;
use lsp_types::notification::Notification as _;
use lsp_types::Diagnostic;
use lsp_types::InitializeParams;
use lsp_types::InitializeResult;
use lsp_types::ServerInfo;
use std::env;
use std::time::Instant;

pub fn start() -> Result<(), AnyError> {
  info!("Starting Deno language server...");

  let (connection, io_threads) = Connection::stdio();
  let (initialize_id, initialize_params) = connection.initialize_start()?;
  let initialize_params: InitializeParams =
    serde_json::from_value(initialize_params)?;

  let capabilities =
    capabilities::server_capabilities(&initialize_params.capabilities);

  let version = format!(
    "{} ({}, {})",
    crate::version::deno(),
    env!("PROFILE"),
    env!("TARGET")
  );

  info!("  version: {}", version);

  let initialize_result = InitializeResult {
    capabilities,
    server_info: Some(ServerInfo {
      name: "deno-language-server".to_string(),
      version: Some(version),
    }),
  };
  let initialize_result = serde_json::to_value(initialize_result)?;

  connection.initialize_finish(initialize_id, initialize_result)?;

  if let Some(client_info) = initialize_params.client_info {
    info!(
      "Connected to \"{}\" {}",
      client_info.name,
      client_info.version.unwrap_or_default()
    );
  }

  let mut config = Config::default();
  config.root_uri = initialize_params.root_uri.clone();
  if let Some(value) = initialize_params.initialization_options {
    config.update(value)?;
  }
  config.update_capabilities(&initialize_params.capabilities);

  let mut server_state = state::ServerState::new(connection.sender, config);

  // TODO(@kitsonk) need to make this configurable, respect unstable
  let ts_config = TsConfig::new(json!({
    "allowJs": true,
    "experimentalDecorators": true,
    "isolatedModules": true,
    "lib": ["deno.ns", "deno.window"],
    "module": "esnext",
    "noEmit": true,
    "strict": true,
    "target": "esnext",
  }));
  let state = server_state.snapshot();
  tsc::request(
    &mut server_state.ts_runtime,
    &state,
    tsc::RequestMethod::Configure(ts_config),
  )?;

  // listen for events and run the main loop
  server_state.run(connection.receiver)?;

  io_threads.join()?;
  info!("Stop language server");
  Ok(())
}

impl ServerState {
  fn handle_event(&mut self, event: Event) -> Result<(), AnyError> {
    let received = Instant::now();
    debug!("handle_event({:?})", event);

    match event {
      Event::Message(message) => match message {
        Message::Request(request) => self.on_request(request, received)?,
        Message::Notification(notification) => {
          self.on_notification(notification)?
        }
        Message::Response(response) => self.complete_request(response),
      },
      Event::Task(mut task) => loop {
        match task {
          Task::Response(response) => self.respond(response),
          Task::Diagnostics((source, diagnostics_per_file)) => {
            for (file_id, version, diagnostics) in diagnostics_per_file {
              self.diagnostics.set(
                file_id,
                source.clone(),
                version,
                diagnostics,
              );
            }
          }
        }

        task = match self.task_receiver.try_recv() {
          Ok(task) => task,
          Err(_) => break,
        };
      },
    }

    // process server sent notifications, like diagnostics
    // TODO(@kitsonk) currently all of these refresh all open documents, though
    // in a lot of cases, like linting, we would only care about the files
    // themselves that have changed
    if self.process_changes() {
      debug!("process changes");
      let state = self.snapshot();
      self.spawn(move || {
        let diagnostics = diagnostics::generate_linting_diagnostics(&state);
        Task::Diagnostics((DiagnosticSource::Lint, diagnostics))
      });
      // TODO(@kitsonk) isolates do not have Send to be safely sent between
      // threads, so I am not sure this is the best way to handle queuing up of
      // getting the diagnostics from the isolate.
      let state = self.snapshot();
      let diagnostics =
        diagnostics::generate_ts_diagnostics(&state, &mut self.ts_runtime)?;
      self.spawn(move || {
        Task::Diagnostics((DiagnosticSource::TypeScript, diagnostics))
      });
    }

    // process any changes to the diagnostics
    if let Some(diagnostic_changes) = self.diagnostics.take_changes() {
      debug!("diagnostics have changed");
      let state = self.snapshot();
      for file_id in diagnostic_changes {
        let file_cache = state.file_cache.read().unwrap();
        // TODO(@kitsonk) not totally happy with the way we collect and store
        // different types of diagnostics and offer them up to the client, we
        // do need to send "empty" vectors though when a particular feature is
        // disabled, otherwise the client will not clear down previous
        // diagnostics
        let mut diagnostics: Vec<Diagnostic> = if state.config.settings.lint {
          self
            .diagnostics
            .diagnostics_for(file_id, DiagnosticSource::Lint)
            .cloned()
            .collect()
        } else {
          vec![]
        };
        if state.config.settings.enable {
          diagnostics.extend(
            self
              .diagnostics
              .diagnostics_for(file_id, DiagnosticSource::TypeScript)
              .cloned(),
          );
        }
        let specifier = file_cache.get_specifier(file_id);
        let uri = specifier.as_url().clone();
        let version = if let Some(doc_data) = self.doc_data.get(specifier) {
          doc_data.version
        } else {
          None
        };
        self.send_notification::<lsp_types::notification::PublishDiagnostics>(
          lsp_types::PublishDiagnosticsParams {
            uri,
            diagnostics,
            version,
          },
        );
      }
    }

    Ok(())
  }

  fn on_notification(
    &mut self,
    notification: Notification,
  ) -> Result<(), AnyError> {
    NotificationDispatcher {
      notification: Some(notification),
      server_state: self,
    }
    // TODO(@kitsonk) this is just stubbed out and we don't currently actually
    // cancel in progress work, though most of our work isn't long running
    .on::<lsp_types::notification::Cancel>(|state, params| {
      let id: RequestId = match params.id {
        lsp_types::NumberOrString::Number(id) => id.into(),
        lsp_types::NumberOrString::String(id) => id.into(),
      };
      state.cancel(id);
      Ok(())
    })?
    .on::<lsp_types::notification::DidOpenTextDocument>(|state, params| {
      if params.text_document.uri.scheme() == "deno" {
        // we can ignore virtual text documents opening, as they don't need to
        // be tracked in memory, as they are static assets that won't change
        // already managed by the language service
        return Ok(());
      }
      let specifier = utils::normalize_url(params.text_document.uri);
      if state
        .doc_data
        .insert(
          specifier.clone(),
          DocumentData::new(
            specifier.clone(),
            params.text_document.version,
            &params.text_document.text,
            state.maybe_import_map.clone(),
          ),
        )
        .is_some()
      {
        error!("duplicate DidOpenTextDocument: {}", specifier);
      }
      state
        .file_cache
        .write()
        .unwrap()
        .set_contents(specifier, Some(params.text_document.text.into_bytes()));

      Ok(())
    })?
    .on::<lsp_types::notification::DidChangeTextDocument>(|state, params| {
      let specifier = utils::normalize_url(params.text_document.uri);
      let mut file_cache = state.file_cache.write().unwrap();
      let file_id = file_cache.lookup(&specifier).unwrap();
      let mut content = file_cache.get_contents(file_id)?;
      apply_content_changes(&mut content, params.content_changes);
      let doc_data = state.doc_data.get_mut(&specifier).unwrap();
      doc_data.update(
        params.text_document.version,
        &content,
        state.maybe_import_map.clone(),
      );
      file_cache.set_contents(specifier, Some(content.into_bytes()));

      Ok(())
    })?
    .on::<lsp_types::notification::DidCloseTextDocument>(|state, params| {
      if params.text_document.uri.scheme() == "deno" {
        // we can ignore virtual text documents opening, as they don't need to
        // be tracked in memory, as they are static assets that won't change
        // already managed by the language service
        return Ok(());
      }
      let specifier = utils::normalize_url(params.text_document.uri);
      if state.doc_data.remove(&specifier).is_none() {
        error!("orphaned document: {}", specifier);
      }
      // TODO(@kitsonk) should we do garbage collection on the diagnostics?

      Ok(())
    })?
    .on::<lsp_types::notification::DidSaveTextDocument>(|_state, _params| {
      // nothing to do yet... cleanup things?

      Ok(())
    })?
    .on::<lsp_types::notification::DidChangeConfiguration>(|state, _params| {
      state.send_request::<lsp_types::request::WorkspaceConfiguration>(
        lsp_types::ConfigurationParams {
          items: vec![lsp_types::ConfigurationItem {
            scope_uri: None,
            section: Some("deno".to_string()),
          }],
        },
        |state, response| {
          let Response { error, result, .. } = response;

          match (error, result) {
            (Some(err), _) => {
              error!("failed to fetch the extension settings: {:?}", err);
            }
            (None, Some(config)) => {
              if let Some(config) = config.get(0) {
                if let Err(err) = state.config.update(config.clone()) {
                  error!("failed to update settings: {}", err);
                }
                if let Err(err) = update_import_map(state) {
                  state
                    .send_notification::<lsp_types::notification::ShowMessage>(
                      lsp_types::ShowMessageParams {
                        typ: lsp_types::MessageType::Warning,
                        message: err.to_string(),
                      },
                    );
                }
              }
            }
            (None, None) => {
              error!("received empty extension settings from the client");
            }
          }
        },
      );

      Ok(())
    })?
    .on::<lsp_types::notification::DidChangeWatchedFiles>(|state, params| {
      // if the current import map has changed, we need to reload it
      if let Some(import_map_uri) = &state.maybe_import_map_uri {
        if params.changes.iter().any(|fe| import_map_uri == &fe.uri) {
          update_import_map(state)?;
        }
      }
      Ok(())
    })?
    .finish();

    Ok(())
  }

  fn on_request(
    &mut self,
    request: Request,
    received: Instant,
  ) -> Result<(), AnyError> {
    self.register_request(&request, received);

    if self.shutdown_requested {
      self.respond(Response::new_err(
        request.id,
        ErrorCode::InvalidRequest as i32,
        "Shutdown already requested".to_string(),
      ));
      return Ok(());
    }

    if self.status == Status::Loading && request.method != "shutdown" {
      self.respond(Response::new_err(
        request.id,
        ErrorCode::ContentModified as i32,
        "Deno Language Server is still loading...".to_string(),
      ));
      return Ok(());
    }

    RequestDispatcher {
      request: Some(request),
      server_state: self,
    }
    .on_sync::<lsp_types::request::Shutdown>(|s, ()| {
      s.shutdown_requested = true;
      Ok(())
    })?
    .on_sync::<lsp_types::request::DocumentHighlightRequest>(
      handlers::handle_document_highlight,
    )?
    .on_sync::<lsp_types::request::GotoDefinition>(
      handlers::handle_goto_definition,
    )?
    .on_sync::<lsp_types::request::HoverRequest>(handlers::handle_hover)?
    .on_sync::<lsp_types::request::Completion>(handlers::handle_completion)?
    .on_sync::<lsp_types::request::References>(handlers::handle_references)?
    .on::<lsp_types::request::Formatting>(handlers::handle_formatting)
    .on::<lsp_extensions::VirtualTextDocument>(
      handlers::handle_virtual_text_document,
    )
    .finish();

    Ok(())
  }

  /// Start consuming events from the provided receiver channel.
  pub fn run(mut self, inbox: Receiver<Message>) -> Result<(), AnyError> {
    // Check to see if we need to setup the import map
    if let Err(err) = update_import_map(&mut self) {
      self.send_notification::<lsp_types::notification::ShowMessage>(
        lsp_types::ShowMessageParams {
          typ: lsp_types::MessageType::Warning,
          message: err.to_string(),
        },
      );
    }

    // we are going to watch all the JSON files in the workspace, and the
    // notification handler will pick up any of the changes of those files we
    // are interested in.
    let watch_registration_options =
      lsp_types::DidChangeWatchedFilesRegistrationOptions {
        watchers: vec![lsp_types::FileSystemWatcher {
          glob_pattern: "**/*.json".to_string(),
          kind: Some(lsp_types::WatchKind::Change),
        }],
      };
    let registration = lsp_types::Registration {
      id: "workspace/didChangeWatchedFiles".to_string(),
      method: "workspace/didChangeWatchedFiles".to_string(),
      register_options: Some(
        serde_json::to_value(watch_registration_options).unwrap(),
      ),
    };
    self.send_request::<lsp_types::request::RegisterCapability>(
      lsp_types::RegistrationParams {
        registrations: vec![registration],
      },
      |_, _| (),
    );

    self.transition(Status::Ready);

    while let Some(event) = self.next_event(&inbox) {
      if let Event::Message(Message::Notification(notification)) = &event {
        if notification.method == lsp_types::notification::Exit::METHOD {
          return Ok(());
        }
      }
      self.handle_event(event)?
    }

    Err(custom_error(
      "ClientError",
      "Client exited without proper shutdown sequence.",
    ))
  }
}
