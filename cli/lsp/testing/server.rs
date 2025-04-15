// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;

use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_runtime::tokio_util::create_basic_runtime;
use tokio::sync::mpsc;
use tower_lsp::jsonrpc::Error as LspError;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types as lsp;

use super::definitions::TestModule;
use super::execution::TestRun;
use super::lsp_custom;
use crate::lsp::client::Client;
use crate::lsp::client::TestingNotification;
use crate::lsp::config;
use crate::lsp::language_server::StateSnapshot;
use crate::lsp::performance::Performance;
use crate::lsp::urls::url_to_uri;

fn as_delete_notification(
  url: &ModuleSpecifier,
) -> Result<TestingNotification, AnyError> {
  Ok(TestingNotification::DeleteModule(
    lsp_custom::TestModuleDeleteNotificationParams {
      text_document: lsp::TextDocumentIdentifier {
        uri: url_to_uri(url)?,
      },
    },
  ))
}

pub type TestServerTests =
  Arc<tokio::sync::Mutex<HashMap<ModuleSpecifier, (TestModule, String)>>>;

/// The main structure which handles requests and sends notifications related
/// to the Testing API.
#[derive(Debug)]
pub struct TestServer {
  client: Client,
  performance: Arc<Performance>,
  /// A channel for handling run requests from the client
  run_channel: mpsc::UnboundedSender<u32>,
  /// A map of run ids to test runs
  runs: Arc<Mutex<HashMap<u32, TestRun>>>,
  /// Tests that are discovered from a versioned document
  tests: TestServerTests,
  /// A channel for requesting that changes to documents be statically analyzed
  /// for tests
  update_channel: mpsc::UnboundedSender<Arc<StateSnapshot>>,
}

impl TestServer {
  pub fn new(
    client: Client,
    performance: Arc<Performance>,
    maybe_root_url: Option<Arc<Url>>,
  ) -> Self {
    let tests = Default::default();

    let (update_channel, mut update_rx) =
      mpsc::unbounded_channel::<Arc<StateSnapshot>>();
    let (run_channel, mut run_rx) = mpsc::unbounded_channel::<u32>();

    let server = Self {
      client,
      performance,
      run_channel,
      runs: Default::default(),
      tests,
      update_channel,
    };

    let tests = server.tests.clone();
    let client = server.client.clone();
    let performance = server.performance.clone();
    let mru = maybe_root_url.clone();
    let _update_join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();

      runtime.block_on(async {
        loop {
          match update_rx.recv().await {
            None => break,
            Some(snapshot) => {
              let mark = performance.mark("lsp.testing_update");
              let mut tests = tests.lock().await;
              // we create a list of test modules we currently are tracking
              // eliminating any we go over when iterating over the document
              let mut keys: HashSet<ModuleSpecifier> =
                tests.keys().cloned().collect();
              for document in snapshot
                .document_modules
                .documents
                .filtered_docs(|d| d.is_file_like() && d.is_diagnosable())
              {
                let Some(module) =
                  snapshot.document_modules.primary_module(&document)
                else {
                  continue;
                };
                if module.specifier.scheme() != "file" {
                  continue;
                }
                if !snapshot
                  .config
                  .specifier_enabled_for_test(&module.specifier)
                {
                  continue;
                }
                keys.remove(&module.specifier);
                let script_version = document.script_version();
                let valid = if let Some((_, old_script_version)) =
                  tests.get(&module.specifier)
                {
                  old_script_version == &script_version
                } else {
                  false
                };
                if !valid {
                  let was_empty = tests
                    .remove(&module.specifier)
                    .map(|(tm, _)| tm.is_empty())
                    .unwrap_or(true);
                  let test_module = module
                    .test_module()
                    .await
                    .map(|tm| tm.as_ref().clone())
                    .unwrap_or_else(|| {
                      TestModule::new(module.specifier.as_ref().clone())
                    });
                  if !test_module.is_empty() {
                    if let Ok(params) =
                      test_module.as_replace_notification(mru.as_deref())
                    {
                      client.send_test_notification(params);
                    }
                  } else if !was_empty {
                    if let Ok(params) =
                      as_delete_notification(&module.specifier)
                    {
                      client.send_test_notification(params);
                    }
                  }
                  tests.insert(
                    module.specifier.as_ref().clone(),
                    (test_module, script_version),
                  );
                }
              }
              for key in &keys {
                if let Ok(params) = as_delete_notification(key) {
                  client.send_test_notification(params);
                }
              }
              performance.measure(mark);
            }
          }
        }
      })
    });

    let client = server.client.clone();
    let runs = server.runs.clone();
    let _run_join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();

      runtime.block_on(async {
        loop {
          match run_rx.recv().await {
            None => break,
            Some(id) => {
              let maybe_run = {
                let runs = runs.lock();
                runs.get(&id).cloned()
              };
              if let Some(run) = maybe_run {
                match run.exec(&client, maybe_root_url.as_deref()).await {
                  Ok(_) => (),
                  Err(err) => {
                    client.show_message(lsp::MessageType::ERROR, err);
                  }
                }
                client.send_test_notification(TestingNotification::Progress(
                  lsp_custom::TestRunProgressParams {
                    id,
                    message: lsp_custom::TestRunProgressMessage::End,
                  },
                ));
                runs.lock().remove(&id);
              }
            }
          }
        }
      })
    });

    server
  }

  fn enqueue_run(&self, id: u32) -> Result<(), AnyError> {
    self.run_channel.send(id).map_err(|err| err.into())
  }

  /// A request from the client to cancel a test run.
  pub fn run_cancel_request(
    &self,
    params: lsp_custom::TestRunCancelParams,
  ) -> LspResult<Option<Value>> {
    if let Some(run) = self.runs.lock().get(&params.id) {
      run.cancel();
      Ok(Some(json!(true)))
    } else {
      Ok(Some(json!(false)))
    }
  }

  /// A request from the client to start a test run.
  pub async fn run_request(
    &self,
    params: lsp_custom::TestRunRequestParams,
    workspace_settings: config::WorkspaceSettings,
  ) -> LspResult<Option<Value>> {
    let test_run =
      { TestRun::init(&params, self.tests.clone(), workspace_settings).await };
    let enqueued = test_run.as_enqueued().await;
    {
      let mut runs = self.runs.lock();
      runs.insert(params.id, test_run);
    }
    self.enqueue_run(params.id).map_err(|err| {
      log::error!("cannot enqueue run: {}", err);
      LspError::internal_error()
    })?;
    Ok(Some(json!({ "enqueued": enqueued })))
  }

  pub(crate) fn update(
    &self,
    snapshot: Arc<StateSnapshot>,
  ) -> Result<(), AnyError> {
    self.update_channel.send(snapshot).map_err(|err| err.into())
  }
}
