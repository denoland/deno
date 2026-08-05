// Copyright 2018-2026 the Deno authors. MIT license.

//! Fast path for `deno test --filter`: collect-then-run.
//!
//! When a name filter is active, discovering which files contain matching
//! tests does not need the full per-file isolation of a real test run: it
//! only needs each module's top-level code to execute so its `Deno.test()`
//! registrations happen. This module implements that discovery ("collect")
//! phase using a small pool of long-lived collector workers, each of which
//! evaluates test files back-to-back in one shared isolate. Files with at
//! least one matching test are then run through the normal, fully-isolated
//! path, completely unchanged; files with no match are skipped, contributing
//! only their `filtered_out` counts (see #20384).
//!
//! Nothing that happens inside a collector is user-visible: collector stdout,
//! stderr and test events are discarded. Any file whose collection cannot be
//! completed faithfully (top-level throw, top-level `Deno.exit()`, a module
//! already evaluated as a dependency of an earlier file) is sent through the
//! legacy path, which reports it exactly as before.
//!
//! The collect phase itself emits no test events. For a skipped file the run
//! phase in `test_specifiers` sends the register/plan/completed events a
//! legacy run of that file would have produced, in the file's original
//! position, so reporter output (including junit's suite order and tap's
//! per-file comments) is unchanged.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use deno_core::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::located_script_name;
use deno_core::unsync::spawn;
use deno_core::unsync::spawn_blocking;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::deno_io::Stdio;
use deno_runtime::deno_io::StdioPipe;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_permissions::PermissionsOptions;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::tokio_util::create_and_run_current_thread;
use deno_runtime::worker::MainWorker;
use indexmap::IndexMap;

use super::TestContainer;
use super::TestDescriptions;
use super::TestFilter;
use super::channel::create_single_test_event_channel;
use crate::args::CliOptions;
use crate::ops;
use crate::sys::CliSys;
use crate::worker::CliMainWorkerFactory;

/// How a specifier fared in the collect phase.
enum CollectOutcome {
  /// The file must go through the normal fully-isolated run path: either at
  /// least one of its tests matches the filter, or it could not be collected
  /// faithfully (top-level error, top-level `Deno.exit()`, module already
  /// evaluated in the collector, worker creation failure).
  Run,
  /// The file was evaluated and none of its test names match the filter: it
  /// can be skipped, reporting these registered tests as filtered out.
  Skip(TestDescriptions),
}

/// Runs the collect phase over `specifiers` and returns them in the original
/// order, each tagged with `None` when the file has to go through the
/// normal, fully-isolated run path or `Some(descriptions)` when it was fully
/// filtered out and only its registrations need to be reported.
///
/// This function is infallible by design: any error is handled by routing the
/// affected file(s) to the legacy path, which will surface the error to the
/// user exactly as a run without this fast path would have.
#[allow(clippy::too_many_arguments, reason = "collect phase inputs")]
pub async fn collect_specifiers_to_run(
  worker_factory: Arc<CliMainWorkerFactory>,
  cli_options: &Arc<CliOptions>,
  permission_desc_parser: &Arc<RuntimePermissionDescriptorParser<CliSys>>,
  specifiers: Vec<ModuleSpecifier>,
  preload_modules: &[ModuleSpecifier],
  require_modules: &[ModuleSpecifier],
  filter: &TestFilter,
  concurrent_jobs: NonZeroUsize,
) -> Vec<(ModuleSpecifier, Option<TestDescriptions>)> {
  // Group the specifiers by workspace member directory: permissions are
  // resolved per member, and each collector worker holds a single
  // `PermissionsContainer` for its lifetime, so a worker must only collect
  // files of one member.
  let mut groups: IndexMap<
    ModuleSpecifier,
    (PermissionsOptions, Vec<ModuleSpecifier>),
  > = IndexMap::new();
  let outcomes: Arc<Mutex<HashMap<ModuleSpecifier, CollectOutcome>>> =
    Arc::new(Mutex::new(HashMap::with_capacity(specifiers.len())));

  for specifier in &specifiers {
    let member_dir = cli_options.workspace().resolve_member_dir(specifier);
    let dir_url = (**member_dir.dir_url()).clone();
    match groups.entry(dir_url) {
      indexmap::map::Entry::Occupied(mut entry) => {
        entry.get_mut().1.push(specifier.clone());
      }
      indexmap::map::Entry::Vacant(entry) => {
        match cli_options.permissions_options_for_dir(&member_dir) {
          Ok(permissions_options) => {
            entry.insert((permissions_options, vec![specifier.clone()]));
          }
          Err(_) => {
            // Resolving permissions failed; the legacy path will hit (and
            // report) the same error when it runs this file.
            outcomes
              .lock()
              .unwrap()
              .insert(specifier.clone(), CollectOutcome::Run);
          }
        }
      }
    }
  }

  for (_, (permissions_options, group_specifiers)) in groups {
    let worker_count = concurrent_jobs.get().min(group_specifiers.len());
    let queue = Arc::new(Mutex::new(VecDeque::from(group_specifiers)));
    let join_handles = (0..worker_count)
      .map(|_| {
        let worker_factory = worker_factory.clone();
        let permission_desc_parser = permission_desc_parser.clone();
        let permissions_options = permissions_options.clone();
        let preload_modules = preload_modules.to_vec();
        let require_modules = require_modules.to_vec();
        let filter = filter.clone();
        let queue = queue.clone();
        let outcomes = outcomes.clone();
        spawn_blocking(move || {
          create_and_run_current_thread(collector_task(
            worker_factory,
            permission_desc_parser,
            permissions_options,
            preload_modules,
            require_modules,
            filter,
            queue,
            outcomes,
          ))
        })
      })
      .collect::<Vec<_>>();
    // A panicking collector is not fatal: any file it claimed but did not
    // finish has no recorded outcome and defaults to the legacy run path.
    let _ = future::join_all(join_handles).await;
  }

  let mut outcomes = outcomes.lock().unwrap();
  specifiers
    .into_iter()
    .map(|specifier| {
      let skipped = match outcomes.remove(&specifier) {
        Some(CollectOutcome::Skip(descriptions)) => Some(descriptions),
        // `None` covers files a panicking collector claimed but never
        // finished; they conservatively go through the legacy run path.
        Some(CollectOutcome::Run) | None => None,
      };
      (specifier, skipped)
    })
    .collect()
}

/// A single collector worker: pulls specifiers off the shared queue and
/// evaluates them back-to-back in one shared isolate until the queue is
/// empty. A worker that gets poisoned (top-level error or `Deno.exit()`) is
/// recycled: dropped and lazily re-created for the next file.
#[allow(clippy::too_many_arguments, reason = "collector inputs")]
async fn collector_task(
  worker_factory: Arc<CliMainWorkerFactory>,
  permission_desc_parser: Arc<RuntimePermissionDescriptorParser<CliSys>>,
  permissions_options: PermissionsOptions,
  preload_modules: Vec<ModuleSpecifier>,
  require_modules: Vec<ModuleSpecifier>,
  filter: TestFilter,
  queue: Arc<Mutex<VecDeque<ModuleSpecifier>>>,
  outcomes: Arc<Mutex<HashMap<ModuleSpecifier, CollectOutcome>>>,
) {
  let mut worker: Option<MainWorker> = None;
  loop {
    let next = queue.lock().unwrap().pop_front();
    let Some(specifier) = next else {
      break;
    };

    if worker.is_none() {
      match create_collector_worker(
        &worker_factory,
        &permission_desc_parser,
        &permissions_options,
        &specifier,
        preload_modules.clone(),
        require_modules.clone(),
      )
      .await
      {
        Ok(w) => worker = Some(w),
        Err(err) => {
          // Route the file through the legacy path, which will hit (and
          // report) the same error; try to create a fresh worker for the
          // next file.
          log::debug!(
            "Failed to create a test collector worker for {}: {:#}",
            specifier,
            err
          );
          record_outcome(&outcomes, specifier, CollectOutcome::Run);
          continue;
        }
      }
    }

    let outcome = collect_file(worker.as_mut().unwrap(), &specifier).await;
    match outcome {
      FileCollectResult::Collected(descriptions) => {
        let matched = descriptions
          .tests
          .values()
          .any(|desc| filter.includes(&desc.name));
        if matched {
          record_outcome(&outcomes, specifier, CollectOutcome::Run);
        } else {
          record_outcome(
            &outcomes,
            specifier,
            CollectOutcome::Skip(descriptions),
          );
        }
      }
      FileCollectResult::AlreadyEvaluated => {
        // The module was already evaluated in this collector as a dependency
        // of an earlier file, so its registrations were attributed to that
        // file and re-evaluating it here would be a no-op. The legacy path
        // evaluates it in a fresh isolate and reports it correctly. The
        // worker itself is still healthy.
        record_outcome(&outcomes, specifier, CollectOutcome::Run);
      }
      FileCollectResult::Poisoned => {
        // Top-level code threw or called `Deno.exit()`. Report nothing from
        // the collector: the legacy path re-runs the file so error output,
        // uncaught-error events and exit codes are byte-for-byte identical.
        // Recycle the poisoned worker.
        record_outcome(&outcomes, specifier, CollectOutcome::Run);
        worker = None;
      }
    }
  }
}

fn record_outcome(
  outcomes: &Mutex<HashMap<ModuleSpecifier, CollectOutcome>>,
  specifier: ModuleSpecifier,
  outcome: CollectOutcome,
) {
  outcomes.lock().unwrap().insert(specifier, outcome);
}

/// The result of evaluating one file in a collector worker.
enum FileCollectResult {
  /// Module evaluation completed; these are the tests it registered.
  Collected(TestDescriptions),
  /// The module was already evaluated in this worker (it was imported by an
  /// earlier test file), so its registrations cannot be observed separately.
  AlreadyEvaluated,
  /// Evaluation threw or the isolate was terminated by a top-level
  /// `Deno.exit()`; the worker must be recycled.
  Poisoned,
}

async fn collect_file(
  worker: &mut MainWorker,
  specifier: &ModuleSpecifier,
) -> FileCollectResult {
  if worker.js_runtime.has_module(specifier.as_str()) {
    return FileCollectResult::AlreadyEvaluated;
  }

  let op_state = worker.js_runtime.op_state();
  {
    let mut state = op_state.borrow_mut();
    // `op_register_test` attributes registrations to the specifier stored in
    // the op state, so it must be updated for every file this worker
    // evaluates.
    state.put::<ModuleSpecifier>(specifier.clone());
    *state.borrow_mut::<TestContainer>() = TestContainer::default();
  }

  let result = match worker.execute_side_module(specifier).await {
    // A top-level throw in an ES module surfaces as an unhandled rejection on
    // the next event loop poll rather than as a module evaluation error, so
    // poll once to catch it (mirrors `test_specifier_inner`).
    Ok(()) => worker.run_up_to_duration(Duration::from_millis(0)).await,
    Err(err) => Err(err),
  };

  if op_state.borrow().has::<ops::testing::IsolateExitInfo>() {
    // Top-level `Deno.exit()`: `op_test_isolate_exit` terminated the isolate.
    // Clear the termination flag so the worker can be dropped cleanly.
    worker.js_runtime.v8_isolate().cancel_terminate_execution();
    return FileCollectResult::Poisoned;
  }
  if result.is_err() {
    return FileCollectResult::Poisoned;
  }

  let container =
    std::mem::take(&mut *op_state.borrow_mut().borrow_mut::<TestContainer>());
  FileCollectResult::Collected(container.descriptions)
}

/// Creates a long-lived collector worker. Mirrors the parts of
/// `configure_main_worker` that top-level evaluation depends on (test ops,
/// isolate exit handler, preload/require modules) and skips everything that
/// only test execution needs (sanitizers, coverage, snapshot mode).
async fn create_collector_worker(
  worker_factory: &CliMainWorkerFactory,
  permission_desc_parser: &Arc<RuntimePermissionDescriptorParser<CliSys>>,
  permissions_options: &PermissionsOptions,
  first_specifier: &ModuleSpecifier,
  preload_modules: Vec<ModuleSpecifier>,
  require_modules: Vec<ModuleSpecifier>,
) -> Result<MainWorker, AnyError> {
  let permissions_container = PermissionsContainer::new(
    permission_desc_parser.clone(),
    Permissions::from_options(
      permission_desc_parser.as_ref(),
      permissions_options,
    )?,
  );

  // Collectors report nothing: their test events and stdio are discarded.
  // The drain task keeps the channel (and thus the stdio forwarding thread)
  // alive so that top-level output cannot block on a full pipe.
  let (worker_sender, mut drain_receiver) = create_single_test_event_channel();
  spawn(async move { while drain_receiver.recv().await.is_some() {} });

  let mut worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Test,
      first_specifier.clone(),
      preload_modules,
      require_modules,
      permissions_container,
      vec![
        ops::testing::deno_test::init(worker_sender.sender),
        ops::lint::deno_lint_ext_for_test::init(),
        ops::jupyter::deno_jupyter_for_test::init(
          tokio::sync::mpsc::unbounded_channel().0,
          tokio::sync::mpsc::unbounded_channel().0,
        ),
      ],
      Stdio {
        stdin: StdioPipe::inherit(),
        stdout: StdioPipe::file(worker_sender.stdout),
        stderr: StdioPipe::file(worker_sender.stderr),
      },
      None,
    )
    .await?;

  // Install the isolate handle and exit handler before any user code runs so
  // a top-level `Deno.exit()` terminates only this collector worker (compare
  // `configure_main_worker`).
  let isolate_handle = worker.v8_isolate_handle();
  worker
    .op_state()
    .borrow_mut()
    .put(ops::testing::TestIsolateHandle(isolate_handle));
  worker
    .execute_script_static(
      located_script_name!(),
      "Deno[Deno.internal].installTestIsolateExitHandler();",
    )
    .map_err(|e| anyhow!(e))?;

  worker.execute_preload_modules().await?;
  let mut worker = worker.into_main_worker();
  if worker
    .js_runtime
    .op_state()
    .borrow()
    .has::<ops::testing::IsolateExitInfo>()
  {
    // A preload module called `Deno.exit()`; this worker cannot collect.
    worker.js_runtime.v8_isolate().cancel_terminate_execution();
    return Err(anyhow!("preload module called Deno.exit()"));
  }
  Ok(worker)
}
