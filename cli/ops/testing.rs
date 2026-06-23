// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_path_util::url_to_file_path;
use deno_runtime::deno_permissions::ChildPermissionsArg;
use deno_runtime::deno_permissions::OpenAccessKind;
use deno_runtime::deno_permissions::PermissionsContainer;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::tools::test::TestContainer;
use crate::tools::test::TestDescription;
use crate::tools::test::TestEvent;
use crate::tools::test::TestEventSender;
use crate::tools::test::TestFailure;
use crate::tools::test::TestLocation;
use crate::tools::test::TestSnapshotSummary;
use crate::tools::test::TestStepDescription;
use crate::tools::test::TestStepResult;

deno_core::extension!(deno_test,
  ops = [
    op_pledge_test_permissions,
    op_restore_test_permissions,
    op_register_test,
    op_register_test_step,
    op_register_test_hook,
    op_test_get_origin,
    op_test_event_step_wait,
    op_test_event_step_result_ok,
    op_test_event_step_result_ignored,
    op_test_event_step_result_failed,
    op_test_event_exit,
    op_test_isolate_exit,
    op_test_snapshot_in_update_mode,
    op_test_snapshot_read,
    op_test_snapshot_write,
    op_test_event_snapshot_summary,
  ],
  options = {
    sender: TestEventSender,
  },
  state = |state, options| {
    state.put(options.sender);
    state.put(TestContainer::default());
  },
);

/// Set by `op_test_isolate_exit` to record that the current test isolate
/// asked to exit via `Deno.exit()` from outside any test function. The test
/// runner reads this flag after each step to detect that the V8 termination
/// it sees was caused by `Deno.exit()` (rather than, say, the watchdog) and
/// can move on to the next specifier without killing the process.
#[derive(Clone, Copy, Debug)]
pub struct IsolateExitInfo {
  // Kept for debug-printing / future use; the test runner currently only
  // checks for the presence of this struct in `OpState`.
  #[allow(dead_code, reason = "diagnostic field")]
  pub exit_code: i32,
}

/// Holds the test isolate's `v8::IsolateHandle` so that `op_test_isolate_exit`
/// can call `terminate_execution` on it. Stored in `OpState` by the test
/// runner when it creates the worker.
#[derive(Clone)]
pub struct TestIsolateHandle(pub v8::IsolateHandle);

#[derive(Clone)]
struct PermissionsHolder(Uuid, PermissionsContainer);

#[op2(stack_trace)]
#[serde]
pub fn op_pledge_test_permissions(
  state: &mut OpState,
  #[serde] args: ChildPermissionsArg,
) -> Result<Uuid, deno_runtime::deno_permissions::ChildPermissionError> {
  let token = Uuid::new_v4();
  let parent_permissions = state.borrow_mut::<PermissionsContainer>();
  let worker_permissions = parent_permissions.create_child_permissions(args)?;
  let parent_permissions = parent_permissions.clone();

  if state.try_take::<PermissionsHolder>().is_some() {
    panic!("pledge test permissions called before restoring previous pledge");
  }
  state.put::<PermissionsHolder>(PermissionsHolder(token, parent_permissions));

  // NOTE: This call overrides current permission set for the worker
  state.put::<PermissionsContainer>(worker_permissions);

  Ok(token)
}

#[op2]
pub fn op_restore_test_permissions(
  state: &mut OpState,
  #[serde] token: Uuid,
) -> Result<(), JsErrorBox> {
  match state.try_take::<PermissionsHolder>() {
    Some(permissions_holder) => {
      if token != permissions_holder.0 {
        panic!(
          "restore test permissions token does not match the stored token"
        );
      }

      let permissions = permissions_holder.1;
      state.put::<PermissionsContainer>(permissions);
      Ok(())
    }
    _ => Err(JsErrorBox::generic("no permissions to restore")),
  }
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[allow(clippy::too_many_arguments, reason = "op")]
#[op2]
fn op_register_test(
  state: &mut OpState,
  #[scoped] function: v8::Global<v8::Function>,
  #[string] name: String,
  ignore: bool,
  only: bool,
  sanitize_ops: bool,
  sanitize_resources: bool,
  #[string] file_name: String,
  #[smi] line_number: u32,
  #[smi] column_number: u32,
  #[buffer] ret_buf: &mut [u8],
  sanitize_only: bool,
  #[smi] timeout_ms: u32,
  #[smi] retry: Option<u32>,
  #[smi] repeats: Option<u32>,
) -> Result<(), JsErrorBox> {
  if ret_buf.len() != 4 {
    return Err(JsErrorBox::type_error(format!(
      "Invalid ret_buf length: {}",
      ret_buf.len()
    )));
  }
  let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
  let origin = state.borrow::<ModuleSpecifier>().to_string();
  let timeout_ms = if timeout_ms == 0 {
    None
  } else {
    Some(timeout_ms)
  };
  let description = TestDescription {
    id,
    name,
    ignore,
    only,
    sanitize_only,
    sanitize_ops,
    sanitize_resources,
    origin: origin.clone(),
    location: TestLocation {
      file_name,
      line_number,
      column_number,
    },
    timeout_ms,
    retry,
    repeats,
  };
  state
    .borrow_mut::<TestContainer>()
    .register(description, function)?;
  ret_buf.copy_from_slice(&(id as u32).to_le_bytes());
  Ok(())
}

#[op2]
fn op_register_test_hook(
  state: &mut OpState,
  #[string] hook_type: String,
  #[scoped] function: v8::Global<v8::Function>,
) -> Result<(), JsErrorBox> {
  let container = state.borrow_mut::<TestContainer>();
  container.register_hook(hook_type, function);
  Ok(())
}

#[op2]
#[string]
fn op_test_get_origin(state: &mut OpState) -> String {
  state.borrow::<ModuleSpecifier>().to_string()
}

#[op2(fast)]
#[smi]
#[allow(clippy::too_many_arguments, reason = "op")]
fn op_register_test_step(
  state: &mut OpState,
  #[string] name: String,
  #[string] file_name: String,
  #[smi] line_number: u32,
  #[smi] column_number: u32,
  #[smi] level: usize,
  #[smi] parent_id: usize,
  #[smi] root_id: usize,
  #[string] root_name: String,
) -> usize {
  let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
  let origin = state.borrow::<ModuleSpecifier>().to_string();
  let description = TestStepDescription {
    id,
    name,
    origin: origin.clone(),
    location: TestLocation {
      file_name,
      line_number,
      column_number,
    },
    level,
    parent_id,
    root_id,
    root_name,
  };
  let sender = state.borrow_mut::<TestEventSender>();
  sender.send(TestEvent::StepRegister(description)).ok();
  id
}

#[op2(fast)]
fn op_test_event_step_wait(state: &mut OpState, #[smi] id: usize) {
  let sender = state.borrow_mut::<TestEventSender>();
  sender.send(TestEvent::StepWait(id)).ok();
}

#[op2(fast)]
fn op_test_event_step_result_ok(
  state: &mut OpState,
  #[smi] id: usize,
  #[smi] duration: u64,
) {
  let sender = state.borrow_mut::<TestEventSender>();
  sender
    .send(TestEvent::StepResult(
      id,
      TestStepResult::Ok,
      Duration::from_millis(duration),
    ))
    .ok();
}

#[op2(fast)]
fn op_test_event_step_result_ignored(
  state: &mut OpState,
  #[smi] id: usize,
  #[smi] duration: u64,
) {
  let sender = state.borrow_mut::<TestEventSender>();
  sender
    .send(TestEvent::StepResult(
      id,
      TestStepResult::Ignored,
      Duration::from_millis(duration),
    ))
    .ok();
}

#[op2]
fn op_test_event_step_result_failed(
  state: &mut OpState,
  #[smi] id: usize,
  #[serde] failure: TestFailure,
  #[smi] duration: u64,
) {
  let sender = state.borrow_mut::<TestEventSender>();
  sender
    .send(TestEvent::StepResult(
      id,
      TestStepResult::Failed(failure),
      Duration::from_millis(duration),
    ))
    .ok();
}

/// Called when a test calls `Deno.exit()` while the exit sanitizer is disabled
/// (`sanitizeExit: false`). Rather than letting the process terminate
/// immediately - which can drop buffered test output - we hand the exit code
/// off to the reporter (running on a separate thread) so it can print a
/// message, flush all output, and then exit the process with the given code.
///
/// This op does not return: once the `Exit` event has been queued, we park the
/// worker so it doesn't keep executing user code (which would otherwise resume
/// after `Deno.exit()` returned) while we wait for the process to be
/// terminated.
#[op2(fast)]
fn op_test_event_exit(state: &mut OpState, #[smi] exit_code: i32) {
  let sender = state.borrow_mut::<TestEventSender>();
  // `TestEvent::Exit` requires stdio sync, so sending it first drains any
  // pending stdout/stderr output to the reporter.
  if sender.send(TestEvent::Exit(exit_code)).is_ok() {
    loop {
      std::thread::park();
    }
  }

  // The channel is closed (the receiver has already finished), so there's
  // nobody left to print a message or flush - just exit directly.
  #[allow(
    clippy::disallowed_methods,
    reason = "a test called Deno.exit() with the exit sanitizer disabled"
  )]
  std::process::exit(exit_code);
}

/// Marker put into `OpState` by the test runner when `deno test` was invoked
/// with `--update-snapshots`. Its presence is what authorizes
/// `op_test_snapshot_write` - JS cannot enable update mode on its own.
pub struct SnapshotUpdateMode;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotLocationArgs {
  dir: Option<String>,
  path: Option<String>,
}

/// Resolves the snapshot file path for the current test origin.
///
/// The default location (`__snapshots__/<test file name>.snap` next to the
/// test file) is derived entirely from the trusted module specifier in
/// `OpState`, so reads and writes to it are exempt from permission checks:
/// the snapshot file is managed by the test runner, like coverage output.
/// When the user overrides the location via the `dir` or `path` option, the
/// path is user-controlled, so a regular read/write permission check is
/// performed by the caller (signalled by the second tuple element).
fn resolve_snapshot_path(
  state: &OpState,
  args: &SnapshotLocationArgs,
) -> Result<(PathBuf, bool), JsErrorBox> {
  let origin = state.borrow::<ModuleSpecifier>();
  let test_file_path = url_to_file_path(origin).map_err(|_| {
    JsErrorBox::type_error(format!(
      "Snapshot testing is not supported for test origin \"{}\"",
      origin
    ))
  })?;
  let test_dir = test_file_path.parent().ok_or_else(|| {
    JsErrorBox::generic("Could not resolve test file directory")
  })?;
  let file_name = test_file_path
    .file_name()
    .ok_or_else(|| JsErrorBox::generic("Could not resolve test file name"))?
    .to_string_lossy()
    .into_owned();
  Ok(match (&args.path, &args.dir) {
    (Some(path), _) => (test_dir.join(path), true),
    (None, Some(dir)) => {
      (test_dir.join(dir).join(format!("{}.snap", file_name)), true)
    }
    (None, None) => (
      test_dir
        .join("__snapshots__")
        .join(format!("{}.snap", file_name)),
      false,
    ),
  })
}

#[op2(fast)]
fn op_test_snapshot_in_update_mode(state: &mut OpState) -> bool {
  state.has::<SnapshotUpdateMode>()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotReadResult {
  path: String,
  content: Option<String>,
}

#[op2]
#[serde]
fn op_test_snapshot_read(
  state: &mut OpState,
  #[serde] args: SnapshotLocationArgs,
) -> Result<SnapshotReadResult, JsErrorBox> {
  let (path, is_custom_location) = resolve_snapshot_path(state, &args)?;
  let path = if is_custom_location {
    let permissions = state.borrow::<PermissionsContainer>();
    permissions
      .check_open(
        Cow::Owned(path),
        OpenAccessKind::Read,
        Some("Deno.TestContext.assertSnapshot"),
      )
      .map_err(JsErrorBox::from_err)?
      .into_owned_path()
  } else {
    path
  };
  let content = match std::fs::read_to_string(&path) {
    Ok(content) => Some(content),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
    Err(err) => {
      return Err(JsErrorBox::generic(format!(
        "Failed to read snapshot file \"{}\": {}",
        path.display(),
        err
      )));
    }
  };
  Ok(SnapshotReadResult {
    path: path.to_string_lossy().into_owned(),
    content,
  })
}

#[op2]
fn op_test_snapshot_write(
  state: &mut OpState,
  #[serde] args: SnapshotLocationArgs,
  #[string] content: String,
) -> Result<(), JsErrorBox> {
  if !state.has::<SnapshotUpdateMode>() {
    return Err(JsErrorBox::generic(
      "Snapshot files can only be written when running with --update-snapshots",
    ));
  }
  let (path, is_custom_location) = resolve_snapshot_path(state, &args)?;
  let path = if is_custom_location {
    let permissions = state.borrow::<PermissionsContainer>();
    permissions
      .check_open(
        Cow::Owned(path),
        OpenAccessKind::Write,
        Some("Deno.TestContext.assertSnapshot"),
      )
      .map_err(JsErrorBox::from_err)?
      .into_owned_path()
  } else {
    path
  };
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent).map_err(|err| {
      JsErrorBox::generic(format!(
        "Failed to create snapshot directory \"{}\": {}",
        parent.display(),
        err
      ))
    })?;
  }
  std::fs::write(&path, content).map_err(|err| {
    JsErrorBox::generic(format!(
      "Failed to write snapshot file \"{}\": {}",
      path.display(),
      err
    ))
  })
}

#[op2]
fn op_test_event_snapshot_summary(
  state: &mut OpState,
  #[smi] updated: usize,
  #[serde] removed: Vec<String>,
) {
  let sender = state.borrow_mut::<TestEventSender>();
  sender
    .send(TestEvent::SnapshotSummary(TestSnapshotSummary {
      updated,
      removed,
    }))
    .ok();
}

/// Called when user code in a test isolate calls `Deno.exit()` outside of any
/// running test function (top-level code, an unload listener, or in async
/// code that the test left running). Instead of terminating the deno
/// process, we record the exit code, notify the reporter, and ask V8 to
/// terminate the current isolate so the test runner can move on to the
/// next specifier.
#[op2(fast)]
fn op_test_isolate_exit(state: &mut OpState, #[smi] exit_code: i32) {
  let origin = state.borrow::<ModuleSpecifier>().to_string();
  let isolate_handle = state.borrow::<TestIsolateHandle>().0.clone();
  state.put(IsolateExitInfo { exit_code });
  let sender = state.borrow_mut::<TestEventSender>();
  // `IsolateExit` requires stdio sync, so sending it first drains any
  // pending stdout/stderr output to the reporter.
  let _ = sender.send(TestEvent::IsolateExit(origin, exit_code));
  // Ask V8 to halt execution. After this op returns, the next bytecode
  // boundary throws an uncatchable termination exception that propagates
  // up through user code into the Rust test runner.
  isolate_handle.terminate_execution();
}
