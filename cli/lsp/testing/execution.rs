// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use super::definitions::TestDefinition;
use super::definitions::TestDefinitions;
use super::lsp_custom;

use crate::args::flags_from_vec;
use crate::args::DenoSubcommand;
use crate::checksum;
use crate::create_main_worker;
use crate::located_script_name;
use crate::lsp::client::Client;
use crate::lsp::client::TestingNotification;
use crate::lsp::config;
use crate::lsp::logging::lsp_log;
use crate::ops;
use crate::proc_state;
use crate::tools::test;
use crate::tools::test::TestEventSender;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::StreamExt;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::RwLock;
use deno_core::ModuleSpecifier;
use deno_runtime::ops::io::Stdio;
use deno_runtime::ops::io::StdioPipe;
use deno_runtime::permissions::Permissions;
use deno_runtime::tokio_util::run_local;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

/// Logic to convert a test request into a set of test modules to be tested and
/// any filters to be applied to those tests
fn as_queue_and_filters(
  params: &lsp_custom::TestRunRequestParams,
  tests: &HashMap<ModuleSpecifier, TestDefinitions>,
) -> (
  HashSet<ModuleSpecifier>,
  HashMap<ModuleSpecifier, LspTestFilter>,
) {
  let mut queue: HashSet<ModuleSpecifier> = HashSet::new();
  let mut filters: HashMap<ModuleSpecifier, LspTestFilter> = HashMap::new();

  if let Some(include) = &params.include {
    for item in include {
      if let Some(test_definitions) = tests.get(&item.text_document.uri) {
        queue.insert(item.text_document.uri.clone());
        if let Some(id) = &item.id {
          if let Some(test) = test_definitions.get_by_id(id) {
            let filter =
              filters.entry(item.text_document.uri.clone()).or_default();
            if let Some(include) = filter.include.as_mut() {
              include.insert(test.id.clone(), test.clone());
            } else {
              let mut include = HashMap::new();
              include.insert(test.id.clone(), test.clone());
              filter.include = Some(include);
            }
          }
        }
      }
    }
  }

  // if we didn't have any specific include filters, we assume that all modules
  // will be tested
  if queue.is_empty() {
    queue.extend(tests.keys().cloned());
  }

  for item in &params.exclude {
    if let Some(test_definitions) = tests.get(&item.text_document.uri) {
      if let Some(id) = &item.id {
        // there is no way to exclude a test step
        if item.step_id.is_none() {
          if let Some(test) = test_definitions.get_by_id(id) {
            let filter =
              filters.entry(item.text_document.uri.clone()).or_default();
            filter.exclude.insert(test.id.clone(), test.clone());
          }
        }
      } else {
        // the entire test module is excluded
        queue.remove(&item.text_document.uri);
      }
    }
  }

  (queue, filters)
}

fn as_test_messages<S: AsRef<str>>(
  message: S,
  is_markdown: bool,
) -> Vec<lsp_custom::TestMessage> {
  let message = lsp::MarkupContent {
    kind: if is_markdown {
      lsp::MarkupKind::Markdown
    } else {
      lsp::MarkupKind::PlainText
    },
    value: message.as_ref().to_string(),
  };
  vec![lsp_custom::TestMessage {
    message,
    expected_output: None,
    actual_output: None,
    location: None,
  }]
}

#[derive(Debug, Clone, Default, PartialEq)]
struct LspTestFilter {
  include: Option<HashMap<String, TestDefinition>>,
  exclude: HashMap<String, TestDefinition>,
}

impl LspTestFilter {
  fn as_ids(&self, test_definitions: &TestDefinitions) -> Vec<String> {
    let ids: Vec<String> = if let Some(include) = &self.include {
      include.keys().cloned().collect()
    } else {
      test_definitions
        .discovered
        .iter()
        .map(|td| td.id.clone())
        .collect()
    };
    ids
      .into_iter()
      .filter(|id| !self.exclude.contains_key(id))
      .collect()
  }
}

async fn test_specifier(
  ps: proc_state::ProcState,
  permissions: Permissions,
  specifier: ModuleSpecifier,
  mode: test::TestMode,
  sender: &TestEventSender,
  token: CancellationToken,
  filter: test::TestFilter,
) -> Result<(), AnyError> {
  if !token.is_cancelled() {
    let mut worker = create_main_worker(
      &ps,
      specifier.clone(),
      permissions,
      vec![ops::testing::init(sender.clone(), filter)],
      Stdio {
        stdin: StdioPipe::Inherit,
        stdout: StdioPipe::File(sender.stdout()),
        stderr: StdioPipe::File(sender.stderr()),
      },
    );

    worker.js_runtime.execute_script(
      &located_script_name!(),
      r#"Deno[Deno.internal].enableTestAndBench()"#,
    )?;

    worker
      .execute_script(
        &located_script_name!(),
        "Deno.core.enableOpCallTracing();",
      )
      .unwrap();

    if mode != test::TestMode::Documentation {
      worker.execute_side_module(&specifier).await?;
    }

    worker.dispatch_load_event(&located_script_name!())?;

    let test_result = worker.js_runtime.execute_script(
      &located_script_name!(),
      r#"Deno[Deno.internal].runTests()"#,
    )?;

    worker.js_runtime.resolve_value(test_result).await?;

    loop {
      if !worker.dispatch_beforeunload_event(&located_script_name!())? {
        break;
      }
      worker.run_event_loop(false).await?;
    }
    worker.dispatch_unload_event(&located_script_name!())?;
  }

  Ok(())
}

#[derive(Debug, Clone)]
pub struct TestRun {
  id: u32,
  kind: lsp_custom::TestRunKind,
  filters: HashMap<ModuleSpecifier, LspTestFilter>,
  queue: HashSet<ModuleSpecifier>,
  tests: Arc<Mutex<HashMap<ModuleSpecifier, TestDefinitions>>>,
  token: CancellationToken,
  workspace_settings: config::WorkspaceSettings,
}

impl TestRun {
  pub fn new(
    params: &lsp_custom::TestRunRequestParams,
    tests: Arc<Mutex<HashMap<ModuleSpecifier, TestDefinitions>>>,
    workspace_settings: config::WorkspaceSettings,
  ) -> Self {
    let (queue, filters) = {
      let tests = tests.lock();
      as_queue_and_filters(params, &tests)
    };

    Self {
      id: params.id,
      kind: params.kind.clone(),
      filters,
      queue,
      tests,
      token: CancellationToken::new(),
      workspace_settings,
    }
  }

  /// Provide the tests of a test run as an enqueued module which can be sent
  /// to the client to indicate tests are enqueued for testing.
  pub fn as_enqueued(&self) -> Vec<lsp_custom::EnqueuedTestModule> {
    let tests = self.tests.lock();
    self
      .queue
      .iter()
      .map(|s| {
        let ids = if let Some(test_definitions) = tests.get(s) {
          if let Some(filter) = self.filters.get(s) {
            filter.as_ids(test_definitions)
          } else {
            test_definitions
              .discovered
              .iter()
              .map(|test| test.id.clone())
              .collect()
          }
        } else {
          Vec::new()
        };
        lsp_custom::EnqueuedTestModule {
          text_document: lsp::TextDocumentIdentifier { uri: s.clone() },
          ids,
        }
      })
      .collect()
  }

  /// If being executed, cancel the test.
  pub fn cancel(&self) {
    self.token.cancel();
  }

  /// Execute the tests, dispatching progress notifications to the client.
  pub async fn exec(
    &self,
    client: &Client,
    maybe_root_uri: Option<&ModuleSpecifier>,
  ) -> Result<(), AnyError> {
    let args = self.get_args();
    lsp_log!("Executing test run with arguments: {}", args.join(" "));
    let flags = flags_from_vec(args.into_iter().map(String::from).collect())?;
    let ps = proc_state::ProcState::build(flags).await?;
    let permissions =
      Permissions::from_options(&ps.options.permissions_options());
    test::check_specifiers(
      &ps,
      permissions.clone(),
      self
        .queue
        .iter()
        .map(|s| (s.clone(), test::TestMode::Executable))
        .collect(),
    )
    .await?;

    let (sender, mut receiver) = mpsc::unbounded_channel::<test::TestEvent>();
    let sender = TestEventSender::new(sender);

    let (concurrent_jobs, fail_fast) =
      if let DenoSubcommand::Test(test_flags) = ps.options.sub_command() {
        (
          test_flags.concurrent_jobs.into(),
          test_flags.fail_fast.map(|count| count.into()),
        )
      } else {
        unreachable!("Should always be Test subcommand.");
      };

    let mut queue = self.queue.iter().collect::<Vec<&ModuleSpecifier>>();
    queue.sort();

    let tests: Arc<RwLock<IndexMap<usize, test::TestDescription>>> =
      Arc::new(RwLock::new(IndexMap::new()));
    let mut test_steps = IndexMap::new();

    let tests_ = tests.clone();
    let join_handles = queue.into_iter().map(move |specifier| {
      let specifier = specifier.clone();
      let ps = ps.clone();
      let permissions = permissions.clone();
      let mut sender = sender.clone();
      let lsp_filter = self.filters.get(&specifier);
      let filter = test::TestFilter {
        substring: None,
        regex: None,
        include: lsp_filter.and_then(|f| {
          f.include
            .as_ref()
            .map(|i| i.values().map(|t| t.name.clone()).collect())
        }),
        exclude: lsp_filter
          .map(|f| f.exclude.values().map(|t| t.name.clone()).collect())
          .unwrap_or_default(),
      };
      let token = self.token.clone();
      let tests = tests_.clone();

      tokio::task::spawn_blocking(move || {
        let origin = specifier.to_string();
        let file_result = run_local(test_specifier(
          ps,
          permissions,
          specifier,
          test::TestMode::Executable,
          &sender,
          token,
          filter,
        ));
        if let Err(error) = file_result {
          if error.is::<JsError>() {
            sender.send(test::TestEvent::UncaughtError(
              origin.clone(),
              Box::new(error.downcast::<JsError>().unwrap()),
            ))?;
            for desc in tests.read().values() {
              if desc.origin == origin {
                sender.send(test::TestEvent::Result(
                  desc.id,
                  test::TestResult::Cancelled,
                  0,
                ))?
              }
            }
          } else {
            return Err(error);
          }
        }
        Ok(())
      })
    });

    let join_stream = stream::iter(join_handles)
      .buffer_unordered(concurrent_jobs)
      .collect::<Vec<Result<Result<(), AnyError>, tokio::task::JoinError>>>();

    let mut reporter = Box::new(LspTestReporter::new(
      self,
      client.clone(),
      maybe_root_uri,
      self.tests.clone(),
    ));

    let handler = {
      tokio::task::spawn(async move {
        let earlier = Instant::now();
        let mut summary = test::TestSummary::new();
        let mut used_only = false;

        while let Some(event) = receiver.recv().await {
          match event {
            test::TestEvent::Register(description) => {
              reporter.report_register(&description);
              tests.write().insert(description.id, description);
            }
            test::TestEvent::Plan(plan) => {
              summary.total += plan.total;
              summary.filtered_out += plan.filtered_out;

              if plan.used_only {
                used_only = true;
              }

              reporter.report_plan(&plan);
            }
            test::TestEvent::Wait(id) => {
              reporter.report_wait(tests.read().get(&id).unwrap());
            }
            test::TestEvent::Output(output) => {
              reporter.report_output(&output);
            }
            test::TestEvent::Result(id, result, elapsed) => {
              let description = tests.read().get(&id).unwrap().clone();
              match &result {
                test::TestResult::Ok => summary.passed += 1,
                test::TestResult::Ignored => summary.ignored += 1,
                test::TestResult::Failed(error) => {
                  summary.failed += 1;
                  summary.failures.push((description.clone(), error.clone()));
                }
                test::TestResult::Cancelled => {
                  summary.failed += 1;
                }
              }

              reporter.report_result(&description, &result, elapsed);
            }
            test::TestEvent::UncaughtError(origin, error) => {
              reporter.report_uncaught_error(&origin, &error);
              summary.failed += 1;
              summary.uncaught_errors.push((origin, error));
            }
            test::TestEvent::StepRegister(description) => {
              reporter.report_step_register(&description);
              test_steps.insert(description.id, description);
            }
            test::TestEvent::StepWait(id) => {
              reporter.report_step_wait(test_steps.get(&id).unwrap());
            }
            test::TestEvent::StepResult(id, result, duration) => {
              match &result {
                test::TestStepResult::Ok => {
                  summary.passed_steps += 1;
                }
                test::TestStepResult::Ignored => {
                  summary.ignored_steps += 1;
                }
                test::TestStepResult::Failed(_) => {
                  summary.failed_steps += 1;
                }
                test::TestStepResult::Pending(_) => {
                  summary.pending_steps += 1;
                }
              }
              reporter.report_step_result(
                test_steps.get(&id).unwrap(),
                &result,
                duration,
              );
            }
          }

          if let Some(count) = fail_fast {
            if summary.failed >= count {
              break;
            }
          }
        }

        let elapsed = Instant::now().duration_since(earlier);
        reporter.report_summary(&summary, &elapsed);

        if used_only {
          return Err(anyhow!(
            "Test failed because the \"only\" option was used"
          ));
        }

        if summary.failed > 0 {
          return Err(anyhow!("Test failed"));
        }

        Ok(())
      })
    };

    let (join_results, result) = future::join(join_stream, handler).await;

    // propagate any errors
    for join_result in join_results {
      join_result??;
    }

    result??;

    Ok(())
  }

  fn get_args(&self) -> Vec<&str> {
    let mut args = vec!["deno", "test"];
    args.extend(
      self
        .workspace_settings
        .testing
        .args
        .iter()
        .map(|s| s.as_str()),
    );
    if self.workspace_settings.unstable && !args.contains(&"--unstable") {
      args.push("--unstable");
    }
    if let Some(config) = &self.workspace_settings.config {
      if !args.contains(&"--config") && !args.contains(&"-c") {
        args.push("--config");
        args.push(config.as_str());
      }
    }
    if let Some(import_map) = &self.workspace_settings.import_map {
      if !args.contains(&"--import-map") {
        args.push("--import-map");
        args.push(import_map.as_str());
      }
    }
    if self.kind == lsp_custom::TestRunKind::Debug
      && !args.contains(&"--inspect")
      && !args.contains(&"--inspect-brk")
    {
      args.push("--inspect");
    }
    args
  }
}

#[derive(Debug, PartialEq)]
enum TestOrTestStepDescription {
  TestDescription(test::TestDescription),
  TestStepDescription(test::TestStepDescription),
}

impl From<&test::TestDescription> for TestOrTestStepDescription {
  fn from(desc: &test::TestDescription) -> Self {
    Self::TestDescription(desc.clone())
  }
}

impl From<&test::TestStepDescription> for TestOrTestStepDescription {
  fn from(desc: &test::TestStepDescription) -> Self {
    Self::TestStepDescription(desc.clone())
  }
}

impl From<&TestOrTestStepDescription> for lsp_custom::TestIdentifier {
  fn from(desc: &TestOrTestStepDescription) -> lsp_custom::TestIdentifier {
    match desc {
      TestOrTestStepDescription::TestDescription(test_desc) => test_desc.into(),
      TestOrTestStepDescription::TestStepDescription(test_step_desc) => {
        test_step_desc.into()
      }
    }
  }
}

impl From<&TestOrTestStepDescription> for lsp_custom::TestData {
  fn from(desc: &TestOrTestStepDescription) -> Self {
    match desc {
      TestOrTestStepDescription::TestDescription(desc) => desc.into(),
      TestOrTestStepDescription::TestStepDescription(desc) => desc.into(),
    }
  }
}

impl From<&test::TestDescription> for lsp_custom::TestData {
  fn from(desc: &test::TestDescription) -> Self {
    Self {
      id: desc.static_id(),
      label: desc.name.clone(),
      steps: Default::default(),
      range: None,
    }
  }
}

impl From<&test::TestDescription> for lsp_custom::TestIdentifier {
  fn from(desc: &test::TestDescription) -> Self {
    let uri = ModuleSpecifier::parse(&desc.origin).unwrap();
    Self {
      text_document: lsp::TextDocumentIdentifier { uri },
      id: Some(desc.static_id()),
      step_id: None,
    }
  }
}

impl From<&test::TestStepDescription> for lsp_custom::TestData {
  fn from(desc: &test::TestStepDescription) -> Self {
    Self {
      id: desc.static_id(),
      label: desc.name.clone(),
      steps: Default::default(),
      range: None,
    }
  }
}

impl From<&test::TestStepDescription> for lsp_custom::TestIdentifier {
  fn from(desc: &test::TestStepDescription) -> Self {
    let uri = ModuleSpecifier::parse(&desc.origin).unwrap();
    Self {
      text_document: lsp::TextDocumentIdentifier { uri },
      id: Some(checksum::gen(&[
        desc.origin.as_bytes(),
        desc.root_name.as_bytes(),
      ])),
      step_id: Some(desc.static_id()),
    }
  }
}

struct LspTestReporter {
  client: Client,
  current_origin: Option<String>,
  maybe_root_uri: Option<ModuleSpecifier>,
  id: u32,
  stack: HashMap<String, Vec<TestOrTestStepDescription>>,
  tests: Arc<Mutex<HashMap<ModuleSpecifier, TestDefinitions>>>,
}

impl LspTestReporter {
  fn new(
    run: &TestRun,
    client: Client,
    maybe_root_uri: Option<&ModuleSpecifier>,
    tests: Arc<Mutex<HashMap<ModuleSpecifier, TestDefinitions>>>,
  ) -> Self {
    Self {
      client,
      current_origin: None,
      maybe_root_uri: maybe_root_uri.cloned(),
      id: run.id,
      stack: HashMap::new(),
      tests,
    }
  }

  fn progress(&self, message: lsp_custom::TestRunProgressMessage) {
    self
      .client
      .send_test_notification(TestingNotification::Progress(
        lsp_custom::TestRunProgressParams {
          id: self.id,
          message,
        },
      ));
  }

  fn report_plan(&mut self, _plan: &test::TestPlan) {}

  fn report_register(&mut self, desc: &test::TestDescription) {
    let mut tests = self.tests.lock();
    let tds = tests
      .entry(ModuleSpecifier::parse(&desc.location.file_name).unwrap())
      .or_default();
    if tds.inject(desc.into()) {
      let specifier = ModuleSpecifier::parse(&desc.origin).unwrap();
      let label = if let Some(root) = &self.maybe_root_uri {
        specifier.as_str().replace(root.as_str(), "")
      } else {
        specifier
          .path_segments()
          .and_then(|s| s.last().map(|s| s.to_string()))
          .unwrap_or_else(|| "<unknown>".to_string())
      };
      self
        .client
        .send_test_notification(TestingNotification::Module(
          lsp_custom::TestModuleNotificationParams {
            text_document: lsp::TextDocumentIdentifier { uri: specifier },
            kind: lsp_custom::TestModuleNotificationKind::Insert,
            label,
            tests: vec![desc.into()],
          },
        ));
    }
  }

  fn report_wait(&mut self, desc: &test::TestDescription) {
    self.current_origin = Some(desc.origin.clone());
    let test: lsp_custom::TestIdentifier = desc.into();
    let stack = self.stack.entry(desc.origin.clone()).or_default();
    assert!(stack.is_empty());
    stack.push(desc.into());
    self.progress(lsp_custom::TestRunProgressMessage::Started { test });
  }

  fn report_output(&mut self, output: &[u8]) {
    let test = self.current_origin.as_ref().and_then(|origin| {
      self
        .stack
        .get(origin)
        .and_then(|v| v.last().map(|td| td.into()))
    });
    let value = String::from_utf8_lossy(output).replace('\n', "\r\n");

    self.progress(lsp_custom::TestRunProgressMessage::Output {
      value,
      test,
      // TODO(@kitsonk) test output should include a location
      location: None,
    })
  }

  fn report_result(
    &mut self,
    desc: &test::TestDescription,
    result: &test::TestResult,
    elapsed: u64,
  ) {
    let stack = self.stack.entry(desc.origin.clone()).or_default();
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.pop(), Some(desc.into()));
    self.current_origin = None;
    match result {
      test::TestResult::Ok => {
        self.progress(lsp_custom::TestRunProgressMessage::Passed {
          test: desc.into(),
          duration: Some(elapsed as u32),
        })
      }
      test::TestResult::Ignored => {
        self.progress(lsp_custom::TestRunProgressMessage::Skipped {
          test: desc.into(),
        })
      }
      test::TestResult::Failed(js_error) => {
        let err_string = test::format_test_error(js_error);
        self.progress(lsp_custom::TestRunProgressMessage::Failed {
          test: desc.into(),
          messages: as_test_messages(err_string, false),
          duration: Some(elapsed as u32),
        })
      }
      test::TestResult::Cancelled => {
        self.progress(lsp_custom::TestRunProgressMessage::Failed {
          test: desc.into(),
          messages: vec![],
          duration: Some(elapsed as u32),
        })
      }
    }
  }

  fn report_uncaught_error(&mut self, origin: &str, js_error: &JsError) {
    if self.current_origin == Some(origin.to_string()) {
      self.current_origin = None;
    }
    let stack = self.stack.remove(origin).unwrap_or_default();
    let err_string = format!(
      "Uncaught error from {}: {}\nThis error was not caught from a test and caused the test runner to fail on the referenced module.\nIt most likely originated from a dangling promise, event/timeout handler or top-level code.",
      origin,
      test::format_test_error(js_error)
    );
    let messages = as_test_messages(err_string, false);
    for t in stack.iter().rev() {
      match t {
        TestOrTestStepDescription::TestDescription(desc) => {
          self.progress(lsp_custom::TestRunProgressMessage::Failed {
            test: desc.into(),
            messages: messages.clone(),
            duration: None,
          });
        }
        TestOrTestStepDescription::TestStepDescription(desc) => {
          self.progress(lsp_custom::TestRunProgressMessage::Failed {
            test: desc.into(),
            messages: messages.clone(),
            duration: None,
          });
        }
      }
    }
  }

  fn report_step_register(&mut self, desc: &test::TestStepDescription) {
    let mut tests = self.tests.lock();
    let tds = tests
      .entry(ModuleSpecifier::parse(&desc.location.file_name).unwrap())
      .or_default();
    if tds.inject(desc.into()) {
      let specifier = ModuleSpecifier::parse(&desc.origin).unwrap();
      let mut prev: lsp_custom::TestData = desc.into();
      if let Some(stack) = self.stack.get(&desc.origin) {
        for item in stack.iter().rev() {
          let mut data: lsp_custom::TestData = item.into();
          data.steps = vec![prev];
          prev = data;
        }
        let label = if let Some(root) = &self.maybe_root_uri {
          specifier.as_str().replace(root.as_str(), "")
        } else {
          specifier
            .path_segments()
            .and_then(|s| s.last().map(|s| s.to_string()))
            .unwrap_or_else(|| "<unknown>".to_string())
        };
        self
          .client
          .send_test_notification(TestingNotification::Module(
            lsp_custom::TestModuleNotificationParams {
              text_document: lsp::TextDocumentIdentifier { uri: specifier },
              kind: lsp_custom::TestModuleNotificationKind::Insert,
              label,
              tests: vec![prev],
            },
          ));
      }
    }
  }

  fn report_step_wait(&mut self, desc: &test::TestStepDescription) {
    let test: lsp_custom::TestIdentifier = desc.into();
    let stack = self.stack.entry(desc.origin.clone()).or_default();
    self.current_origin = Some(desc.origin.clone());
    assert!(!stack.is_empty());
    stack.push(desc.into());
    self.progress(lsp_custom::TestRunProgressMessage::Started { test });
  }

  fn report_step_result(
    &mut self,
    desc: &test::TestStepDescription,
    result: &test::TestStepResult,
    elapsed: u64,
  ) {
    let stack = self.stack.entry(desc.origin.clone()).or_default();
    assert_eq!(stack.pop(), Some(desc.into()));
    match result {
      test::TestStepResult::Ok => {
        self.progress(lsp_custom::TestRunProgressMessage::Passed {
          test: desc.into(),
          duration: Some(elapsed as u32),
        })
      }
      test::TestStepResult::Ignored => {
        self.progress(lsp_custom::TestRunProgressMessage::Skipped {
          test: desc.into(),
        })
      }
      test::TestStepResult::Failed(js_error) => {
        let messages = if let Some(js_error) = js_error {
          let err_string = test::format_test_error(js_error);
          as_test_messages(err_string, false)
        } else {
          vec![]
        };
        self.progress(lsp_custom::TestRunProgressMessage::Failed {
          test: desc.into(),
          messages,
          duration: Some(elapsed as u32),
        })
      }
      test::TestStepResult::Pending(_) => {
        self.progress(lsp_custom::TestRunProgressMessage::Enqueued {
          test: desc.into(),
        })
      }
    }
  }

  fn report_summary(
    &mut self,
    _summary: &test::TestSummary,
    _elapsed: &Duration,
  ) {
    // there is nothing to do on report_summary
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::lsp::testing::collectors::tests::new_range;
  use deno_core::serde_json::json;

  #[test]
  fn test_as_queue_and_filters() {
    let specifier = ModuleSpecifier::parse("file:///a/file.ts").unwrap();
    let params = lsp_custom::TestRunRequestParams {
      id: 1,
      kind: lsp_custom::TestRunKind::Run,
      include: Some(vec![lsp_custom::TestIdentifier {
        text_document: lsp::TextDocumentIdentifier {
          uri: specifier.clone(),
        },
        id: None,
        step_id: None,
      }]),
      exclude: vec![lsp_custom::TestIdentifier {
        text_document: lsp::TextDocumentIdentifier {
          uri: specifier.clone(),
        },
        id: Some(
          "69d9fe87f64f5b66cb8b631d4fd2064e8224b8715a049be54276c42189ff8f9f"
            .to_string(),
        ),
        step_id: None,
      }],
    };
    let mut tests = HashMap::new();
    let test_def_a = TestDefinition {
      id: "0b7c6bf3cd617018d33a1bf982a08fe088c5bb54fcd5eb9e802e7c137ec1af94"
        .to_string(),
      level: 0,
      name: "test a".to_string(),
      range: new_range(420, 424),
      steps: vec![],
    };
    let test_def_b = TestDefinition {
      id: "69d9fe87f64f5b66cb8b631d4fd2064e8224b8715a049be54276c42189ff8f9f"
        .to_string(),
      level: 0,
      name: "test b".to_string(),
      range: new_range(480, 481),
      steps: vec![],
    };
    let test_definitions = TestDefinitions {
      discovered: vec![test_def_a, test_def_b.clone()],
      injected: vec![],
      script_version: "1".to_string(),
    };
    tests.insert(specifier.clone(), test_definitions.clone());
    let (queue, filters) = as_queue_and_filters(&params, &tests);
    assert_eq!(json!(queue), json!([specifier]));
    let mut exclude = HashMap::new();
    exclude.insert(
      "69d9fe87f64f5b66cb8b631d4fd2064e8224b8715a049be54276c42189ff8f9f"
        .to_string(),
      test_def_b,
    );
    let maybe_filter = filters.get(&specifier);
    assert!(maybe_filter.is_some());
    let filter = maybe_filter.unwrap();
    assert_eq!(
      filter,
      &LspTestFilter {
        include: None,
        exclude,
      }
    );
    assert_eq!(
      filter.as_ids(&test_definitions),
      vec![
        "0b7c6bf3cd617018d33a1bf982a08fe088c5bb54fcd5eb9e802e7c137ec1af94"
          .to_string()
      ]
    );
  }
}
