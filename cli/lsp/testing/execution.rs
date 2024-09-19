// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::definitions::TestDefinition;
use super::definitions::TestModule;
use super::lsp_custom;
use super::server::TestServerTests;

use crate::args::flags_from_vec;
use crate::args::DenoSubcommand;
use crate::factory::CliFactory;
use crate::lsp::client::Client;
use crate::lsp::client::TestingNotification;
use crate::lsp::config;
use crate::lsp::logging::lsp_log;
use crate::lsp::urls::uri_parse_unencoded;
use crate::lsp::urls::uri_to_url;
use crate::lsp::urls::url_to_uri;
use crate::tools::test;
use crate::tools::test::create_test_event_channel;
use crate::tools::test::FailFastTracker;
use crate::tools::test::TestFailureFormatOptions;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::futures::future;
use deno_core::futures::stream;
use deno_core::futures::StreamExt;
use deno_core::parking_lot::RwLock;
use deno_core::unsync::spawn;
use deno_core::unsync::spawn_blocking;
use deno_core::ModuleSpecifier;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::tokio_util::create_and_run_current_thread;
use indexmap::IndexMap;
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

/// Logic to convert a test request into a set of test modules to be tested and
/// any filters to be applied to those tests
fn as_queue_and_filters(
  params: &lsp_custom::TestRunRequestParams,
  tests: &HashMap<ModuleSpecifier, (TestModule, String)>,
) -> (
  HashSet<ModuleSpecifier>,
  HashMap<ModuleSpecifier, LspTestFilter>,
) {
  let mut queue: HashSet<ModuleSpecifier> = HashSet::new();
  let mut filters: HashMap<ModuleSpecifier, LspTestFilter> = HashMap::new();

  if let Some(include) = &params.include {
    for item in include {
      let url = uri_to_url(&item.text_document.uri);
      if let Some((test_definitions, _)) = tests.get(&url) {
        queue.insert(url.clone());
        if let Some(id) = &item.id {
          if let Some(test) = test_definitions.get(id) {
            let filter = filters.entry(url).or_default();
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
  } else {
    queue.extend(tests.keys().cloned());
  }

  for item in &params.exclude {
    let url = uri_to_url(&item.text_document.uri);
    if let Some((test_definitions, _)) = tests.get(&url) {
      if let Some(id) = &item.id {
        // there is no way to exclude a test step
        if item.step_id.is_none() {
          if let Some(test) = test_definitions.get(id) {
            let filter = filters.entry(url.clone()).or_default();
            filter.exclude.insert(test.id.clone(), test.clone());
          }
        }
      } else {
        // the entire test module is excluded
        queue.remove(&url);
      }
    }
  }

  queue.retain(|s| !tests.get(s).unwrap().0.is_empty());

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
  fn as_ids(&self, test_module: &TestModule) -> Vec<String> {
    let ids: Vec<String> = if let Some(include) = &self.include {
      include.keys().cloned().collect()
    } else {
      test_module
        .defs
        .iter()
        .filter(|(_, d)| d.parent_id.is_none())
        .map(|(k, _)| k.clone())
        .collect()
    };
    ids
      .into_iter()
      .filter(|id| !self.exclude.contains_key(id))
      .collect()
  }
}

#[derive(Debug, Clone)]
pub struct TestRun {
  id: u32,
  kind: lsp_custom::TestRunKind,
  filters: HashMap<ModuleSpecifier, LspTestFilter>,
  queue: HashSet<ModuleSpecifier>,
  tests: TestServerTests,
  token: CancellationToken,
  workspace_settings: config::WorkspaceSettings,
}

impl TestRun {
  pub async fn init(
    params: &lsp_custom::TestRunRequestParams,
    tests: TestServerTests,
    workspace_settings: config::WorkspaceSettings,
  ) -> Self {
    let (queue, filters) = {
      let tests = tests.lock().await;
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
  pub async fn as_enqueued(&self) -> Vec<lsp_custom::EnqueuedTestModule> {
    let tests = self.tests.lock().await;
    self
      .queue
      .iter()
      .filter_map(|s| {
        let ids = if let Some((test_module, _)) = tests.get(s) {
          if let Some(filter) = self.filters.get(s) {
            filter.as_ids(test_module)
          } else {
            LspTestFilter::default().as_ids(test_module)
          }
        } else {
          Vec::new()
        };
        Some(lsp_custom::EnqueuedTestModule {
          text_document: lsp::TextDocumentIdentifier {
            uri: url_to_uri(s).ok()?,
          },
          ids,
        })
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
    let flags = Arc::new(flags_from_vec(
      args.into_iter().map(|s| From::from(s.as_ref())).collect(),
    )?);
    let factory = CliFactory::from_flags(flags);
    let cli_options = factory.cli_options()?;
    // Various test files should not share the same permissions in terms of
    // `PermissionsContainer` - otherwise granting/revoking permissions in one
    // file would have impact on other files, which is undesirable.
    let permission_desc_parser = factory.permission_desc_parser()?.clone();
    let permissions = Permissions::from_options(
      permission_desc_parser.as_ref(),
      &cli_options.permissions_options(),
    )?;
    let main_graph_container = factory.main_module_graph_container().await?;
    main_graph_container
      .check_specifiers(&self.queue.iter().cloned().collect::<Vec<_>>(), None)
      .await?;

    let (concurrent_jobs, fail_fast) =
      if let DenoSubcommand::Test(test_flags) = cli_options.sub_command() {
        (
          test_flags
            .concurrent_jobs
            .unwrap_or_else(|| NonZeroUsize::new(1).unwrap())
            .into(),
          test_flags.fail_fast,
        )
      } else {
        unreachable!("Should always be Test subcommand.");
      };

    // TODO(mmastrac): Temporarily limit concurrency in windows testing to avoid named pipe issue:
    // *** Unexpected server pipe failure '"\\\\.\\pipe\\deno_pipe_e30f45c9df61b1e4.1198.222\\0"': 3
    // This is likely because we're hitting some sort of invisible resource limit
    // This limit is both in cli/lsp/testing/execution.rs and cli/tools/test/mod.rs
    #[cfg(windows)]
    let concurrent_jobs = std::cmp::min(concurrent_jobs, 4);

    let (test_event_sender_factory, mut receiver) = create_test_event_channel();
    let fail_fast_tracker = FailFastTracker::new(fail_fast);

    let mut queue = self.queue.iter().collect::<Vec<&ModuleSpecifier>>();
    queue.sort();

    let tests: Arc<RwLock<IndexMap<usize, test::TestDescription>>> =
      Arc::new(RwLock::new(IndexMap::new()));
    let mut test_steps = IndexMap::new();
    let worker_factory =
      Arc::new(factory.create_cli_main_worker_factory().await?);

    let join_handles = queue.into_iter().map(move |specifier| {
      let specifier = specifier.clone();
      let worker_factory = worker_factory.clone();
      let permissions_container = PermissionsContainer::new(
        permission_desc_parser.clone(),
        permissions.clone(),
      );
      let worker_sender = test_event_sender_factory.worker();
      let fail_fast_tracker = fail_fast_tracker.clone();
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

      spawn_blocking(move || {
        if fail_fast_tracker.should_stop() {
          return Ok(());
        }
        if token.is_cancelled() {
          Ok(())
        } else {
          // All JsErrors are handled by test_specifier and piped into the test
          // channel.
          create_and_run_current_thread(test::test_specifier(
            worker_factory,
            permissions_container,
            specifier,
            worker_sender,
            fail_fast_tracker,
            test::TestSpecifierOptions {
              filter,
              shuffle: None,
              trace_leaks: false,
            },
          ))
        }
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
      spawn(async move {
        let earlier = Instant::now();
        let mut summary = test::TestSummary::new();
        let mut tests_with_result = HashSet::new();
        let mut used_only = false;

        while let Some((_, event)) = receiver.recv().await {
          match event {
            test::TestEvent::Register(description) => {
              for (_, description) in description.into_iter() {
                reporter.report_register(description).await;
                // TODO(mmastrac): we shouldn't need to clone here - we can re-use the descriptions
                tests.write().insert(description.id, description.clone());
              }
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
            test::TestEvent::Slow(id, elapsed) => {
              reporter.report_slow(tests.read().get(&id).unwrap(), elapsed);
            }
            test::TestEvent::Result(id, result, elapsed) => {
              if tests_with_result.insert(id) {
                let description = tests.read().get(&id).unwrap().clone();
                match &result {
                  test::TestResult::Ok => summary.passed += 1,
                  test::TestResult::Ignored => summary.ignored += 1,
                  test::TestResult::Failed(error) => {
                    summary.failed += 1;
                    summary
                      .failures
                      .push(((&description).into(), error.clone()));
                  }
                  test::TestResult::Cancelled => {
                    summary.failed += 1;
                  }
                }
                reporter.report_result(&description, &result, elapsed);
              }
            }
            test::TestEvent::UncaughtError(origin, error) => {
              reporter.report_uncaught_error(&origin, &error);
              summary.failed += 1;
              summary.uncaught_errors.push((origin, error));
            }
            test::TestEvent::StepRegister(description) => {
              reporter.report_step_register(&description).await;
              test_steps.insert(description.id, description);
            }
            test::TestEvent::StepWait(id) => {
              reporter.report_step_wait(test_steps.get(&id).unwrap());
            }
            test::TestEvent::StepResult(id, result, duration) => {
              if tests_with_result.insert(id) {
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
                }
                reporter.report_step_result(
                  test_steps.get(&id).unwrap(),
                  &result,
                  duration,
                );
              }
            }
            test::TestEvent::Completed => {
              reporter.report_completed();
            }
            test::TestEvent::ForceEndReport => {}
            test::TestEvent::Sigint => {}
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

  fn get_args(&self) -> Vec<Cow<str>> {
    let mut args = vec![Cow::Borrowed("deno"), Cow::Borrowed("test")];
    args.extend(
      self
        .workspace_settings
        .testing
        .args
        .iter()
        .map(|s| Cow::Borrowed(s.as_str())),
    );
    args.push(Cow::Borrowed("--trace-leaks"));
    for unstable_feature in self.workspace_settings.unstable.as_deref() {
      let flag = format!("--unstable-{unstable_feature}");
      if !args.contains(&Cow::Borrowed(&flag)) {
        args.push(Cow::Owned(flag));
      }
    }
    if let Some(config) = &self.workspace_settings.config {
      if !args.contains(&Cow::Borrowed("--config"))
        && !args.contains(&Cow::Borrowed("-c"))
      {
        args.push(Cow::Borrowed("--config"));
        args.push(Cow::Borrowed(config.as_str()));
      }
    }
    if let Some(import_map) = &self.workspace_settings.import_map {
      if !args.contains(&Cow::Borrowed("--import-map")) {
        args.push(Cow::Borrowed("--import-map"));
        args.push(Cow::Borrowed(import_map.as_str()));
      }
    }
    if self.kind == lsp_custom::TestRunKind::Debug
      && !args.contains(&Cow::Borrowed("--inspect"))
      && !args.contains(&Cow::Borrowed("--inspect-brk"))
    {
      args.push(Cow::Borrowed("--inspect"));
    }
    args
  }
}

#[derive(Debug, PartialEq)]
enum LspTestDescription {
  /// `(desc, static_id)`
  TestDescription(test::TestDescription, String),
  /// `(desc, static_id)`
  TestStepDescription(test::TestStepDescription, String),
}

impl LspTestDescription {
  fn origin(&self) -> &str {
    match self {
      LspTestDescription::TestDescription(d, _) => d.origin.as_str(),
      LspTestDescription::TestStepDescription(d, _) => d.origin.as_str(),
    }
  }

  fn location(&self) -> &test::TestLocation {
    match self {
      LspTestDescription::TestDescription(d, _) => &d.location,
      LspTestDescription::TestStepDescription(d, _) => &d.location,
    }
  }

  fn parent_id(&self) -> Option<usize> {
    match self {
      LspTestDescription::TestDescription(_, _) => None,
      LspTestDescription::TestStepDescription(d, _) => Some(d.parent_id),
    }
  }

  fn static_id(&self) -> &str {
    match self {
      LspTestDescription::TestDescription(_, i) => i,
      LspTestDescription::TestStepDescription(_, i) => i,
    }
  }

  fn as_test_identifier(
    &self,
    tests: &IndexMap<usize, LspTestDescription>,
  ) -> lsp_custom::TestIdentifier {
    let uri = uri_parse_unencoded(&self.location().file_name).unwrap();
    let static_id = self.static_id();
    let mut root_desc = self;
    while let Some(parent_id) = root_desc.parent_id() {
      root_desc = tests.get(&parent_id).unwrap();
    }
    let root_static_id = root_desc.static_id();
    lsp_custom::TestIdentifier {
      text_document: lsp::TextDocumentIdentifier { uri },
      id: Some(root_static_id.to_string()),
      step_id: if static_id == root_static_id {
        None
      } else {
        Some(static_id.to_string())
      },
    }
  }
}

struct LspTestReporter {
  client: Client,
  id: u32,
  maybe_root_uri: Option<ModuleSpecifier>,
  files: TestServerTests,
  tests: IndexMap<usize, LspTestDescription>,
  current_test: Option<usize>,
}

impl LspTestReporter {
  fn new(
    run: &TestRun,
    client: Client,
    maybe_root_uri: Option<&ModuleSpecifier>,
    files: TestServerTests,
  ) -> Self {
    Self {
      client,
      id: run.id,
      maybe_root_uri: maybe_root_uri.cloned(),
      files,
      tests: Default::default(),
      current_test: Default::default(),
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

  async fn report_register(&mut self, desc: &test::TestDescription) {
    let mut files = self.files.lock().await;
    let specifier = ModuleSpecifier::parse(&desc.location.file_name).unwrap();
    let (test_module, _) = files
      .entry(specifier.clone())
      .or_insert_with(|| (TestModule::new(specifier), "1".to_string()));
    let Ok(uri) = url_to_uri(&test_module.specifier) else {
      return;
    };
    let (static_id, is_new) = test_module.register_dynamic(desc);
    self.tests.insert(
      desc.id,
      LspTestDescription::TestDescription(desc.clone(), static_id.clone()),
    );
    if is_new {
      self
        .client
        .send_test_notification(TestingNotification::Module(
          lsp_custom::TestModuleNotificationParams {
            text_document: lsp::TextDocumentIdentifier { uri },
            kind: lsp_custom::TestModuleNotificationKind::Insert,
            label: test_module.label(self.maybe_root_uri.as_ref()),
            tests: vec![test_module.get_test_data(&static_id)],
          },
        ));
    }
  }

  fn report_wait(&mut self, desc: &test::TestDescription) {
    self.current_test = Some(desc.id);
    let desc = self.tests.get(&desc.id).unwrap();
    let test = desc.as_test_identifier(&self.tests);
    self.progress(lsp_custom::TestRunProgressMessage::Started { test });
  }

  fn report_slow(&mut self, _desc: &test::TestDescription, _elapsed: u64) {}

  fn report_output(&mut self, output: &[u8]) {
    let test = self
      .current_test
      .as_ref()
      .map(|id| self.tests.get(id).unwrap().as_test_identifier(&self.tests));
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
    self.current_test = None;
    match result {
      test::TestResult::Ok => {
        let desc = self.tests.get(&desc.id).unwrap();
        self.progress(lsp_custom::TestRunProgressMessage::Passed {
          test: desc.as_test_identifier(&self.tests),
          duration: Some(elapsed as u32),
        })
      }
      test::TestResult::Ignored => {
        let desc = self.tests.get(&desc.id).unwrap();
        self.progress(lsp_custom::TestRunProgressMessage::Skipped {
          test: desc.as_test_identifier(&self.tests),
        })
      }
      test::TestResult::Failed(failure) => {
        let desc = self.tests.get(&desc.id).unwrap();
        self.progress(lsp_custom::TestRunProgressMessage::Failed {
          test: desc.as_test_identifier(&self.tests),
          messages: as_test_messages(
            failure.format(&TestFailureFormatOptions::default()),
            false,
          ),
          duration: Some(elapsed as u32),
        })
      }
      test::TestResult::Cancelled => {
        let desc = self.tests.get(&desc.id).unwrap();
        self.progress(lsp_custom::TestRunProgressMessage::Failed {
          test: desc.as_test_identifier(&self.tests),
          messages: vec![],
          duration: Some(elapsed as u32),
        })
      }
    }
  }

  fn report_uncaught_error(&mut self, origin: &str, js_error: &JsError) {
    self.current_test = None;
    let err_string = format!(
      "Uncaught error from {}: {}\nThis error was not caught from a test and caused the test runner to fail on the referenced module.\nIt most likely originated from a dangling promise, event/timeout handler or top-level code.",
      origin,
      test::fmt::format_test_error(js_error, &TestFailureFormatOptions::default())
    );
    let messages = as_test_messages(err_string, false);
    for desc in self.tests.values().filter(|d| d.origin() == origin) {
      self.progress(lsp_custom::TestRunProgressMessage::Failed {
        test: desc.as_test_identifier(&self.tests),
        messages: messages.clone(),
        duration: None,
      });
    }
  }

  async fn report_step_register(&mut self, desc: &test::TestStepDescription) {
    let mut files = self.files.lock().await;
    let specifier = ModuleSpecifier::parse(&desc.location.file_name).unwrap();
    let (test_module, _) = files
      .entry(specifier.clone())
      .or_insert_with(|| (TestModule::new(specifier), "1".to_string()));
    let Ok(uri) = url_to_uri(&test_module.specifier) else {
      return;
    };
    let (static_id, is_new) = test_module.register_step_dynamic(
      desc,
      self.tests.get(&desc.parent_id).unwrap().static_id(),
    );
    self.tests.insert(
      desc.id,
      LspTestDescription::TestStepDescription(desc.clone(), static_id.clone()),
    );
    if is_new {
      self
        .client
        .send_test_notification(TestingNotification::Module(
          lsp_custom::TestModuleNotificationParams {
            text_document: lsp::TextDocumentIdentifier { uri },
            kind: lsp_custom::TestModuleNotificationKind::Insert,
            label: test_module.label(self.maybe_root_uri.as_ref()),
            tests: vec![test_module.get_test_data(&static_id)],
          },
        ));
    }
  }

  fn report_step_wait(&mut self, desc: &test::TestStepDescription) {
    if self.current_test == Some(desc.parent_id) {
      self.current_test = Some(desc.id);
    }
    let desc = self.tests.get(&desc.id).unwrap();
    let test = desc.as_test_identifier(&self.tests);
    self.progress(lsp_custom::TestRunProgressMessage::Started { test });
  }

  fn report_step_result(
    &mut self,
    desc: &test::TestStepDescription,
    result: &test::TestStepResult,
    elapsed: u64,
  ) {
    if self.current_test == Some(desc.id) {
      self.current_test = Some(desc.parent_id);
    }
    let desc = self.tests.get(&desc.id).unwrap();
    match result {
      test::TestStepResult::Ok => {
        self.progress(lsp_custom::TestRunProgressMessage::Passed {
          test: desc.as_test_identifier(&self.tests),
          duration: Some(elapsed as u32),
        })
      }
      test::TestStepResult::Ignored => {
        self.progress(lsp_custom::TestRunProgressMessage::Skipped {
          test: desc.as_test_identifier(&self.tests),
        })
      }
      test::TestStepResult::Failed(failure) => {
        self.progress(lsp_custom::TestRunProgressMessage::Failed {
          test: desc.as_test_identifier(&self.tests),
          messages: as_test_messages(
            failure.format(&TestFailureFormatOptions::default()),
            false,
          ),
          duration: Some(elapsed as u32),
        })
      }
    }
  }

  fn report_completed(&mut self) {
    // there is nothing to do on report_completed
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
    // Regression test for https://github.com/denoland/vscode_deno/issues/890.
    let non_test_specifier =
      ModuleSpecifier::parse("file:///a/no_tests.ts").unwrap();
    let params = lsp_custom::TestRunRequestParams {
      id: 1,
      kind: lsp_custom::TestRunKind::Run,
      include: Some(vec![
        lsp_custom::TestIdentifier {
          text_document: lsp::TextDocumentIdentifier {
            uri: url_to_uri(&specifier).unwrap(),
          },
          id: None,
          step_id: None,
        },
        lsp_custom::TestIdentifier {
          text_document: lsp::TextDocumentIdentifier {
            uri: url_to_uri(&non_test_specifier).unwrap(),
          },
          id: None,
          step_id: None,
        },
      ]),
      exclude: vec![lsp_custom::TestIdentifier {
        text_document: lsp::TextDocumentIdentifier {
          uri: url_to_uri(&specifier).unwrap(),
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
      name: "test a".to_string(),
      range: Some(new_range(1, 5, 1, 9)),
      is_dynamic: false,
      parent_id: None,
      step_ids: Default::default(),
    };
    let test_def_b = TestDefinition {
      id: "69d9fe87f64f5b66cb8b631d4fd2064e8224b8715a049be54276c42189ff8f9f"
        .to_string(),
      name: "test b".to_string(),
      range: Some(new_range(2, 5, 2, 9)),
      is_dynamic: false,
      parent_id: None,
      step_ids: Default::default(),
    };
    let test_module = TestModule {
      specifier: specifier.clone(),
      defs: vec![
        (test_def_a.id.clone(), test_def_a.clone()),
        (test_def_b.id.clone(), test_def_b.clone()),
      ]
      .into_iter()
      .collect(),
    };
    tests.insert(specifier.clone(), (test_module.clone(), "1".to_string()));
    tests.insert(
      non_test_specifier.clone(),
      (TestModule::new(non_test_specifier), "1".to_string()),
    );
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
      filter.as_ids(&test_module),
      vec![
        "0b7c6bf3cd617018d33a1bf982a08fe088c5bb54fcd5eb9e802e7c137ec1af94"
          .to_string()
      ]
    );
  }
}
