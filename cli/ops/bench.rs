use crate::tools::bench::BenchEvent;
use crate::tools::bench::BenchOutput;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::Extension;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::ops::io::StdFileResource;
use deno_runtime::permissions::create_child_permissions;
use deno_runtime::permissions::ChildPermissionsArg;
use deno_runtime::permissions::Permissions;
use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;
use std::time;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

pub fn init(
  sender: UnboundedSender<BenchEvent>,
  unstable: bool,
  stdout_writer: os_pipe::PipeWriter,
  stderr_writer: os_pipe::PipeWriter,
) -> Extension {
  // todo(dsheret): don't do this? Taking out the writers was necessary to prevent invalid handle panics
  let stdout_writer = Rc::new(RefCell::new(Some(stdout_writer)));
  let stderr_writer = Rc::new(RefCell::new(Some(stderr_writer)));

  Extension::builder()
    .ops(vec![
      op_pledge_test_permissions::decl(),
      op_restore_test_permissions::decl(),
      op_get_bench_origin::decl(),
      op_dispatch_bench_event::decl(),
      op_bench_now::decl(),
      op_bench_check_unstable::decl(),
    ])
    .middleware(|op| match op.name {
      "op_print" => op_print::decl(),
      _ => op,
    })
    .state(move |state| {
      state.resource_table.replace(
        1,
        StdFileResource::stdio(
          &pipe_writer_to_file(&stdout_writer.borrow_mut().take().unwrap()),
          "stdout",
        ),
      );
      state.resource_table.replace(
        2,
        StdFileResource::stdio(
          &pipe_writer_to_file(&stderr_writer.borrow_mut().take().unwrap()),
          "stderr",
        ),
      );
      state.put(sender.clone());
      state.put(Unstable(unstable));
      Ok(())
    })
    .build()
}

#[cfg(windows)]
fn pipe_writer_to_file(writer: &os_pipe::PipeWriter) -> std::fs::File {
  use std::os::windows::prelude::AsRawHandle;
  use std::os::windows::prelude::FromRawHandle;
  unsafe { std::fs::File::from_raw_handle(writer.as_raw_handle()) }
}

#[cfg(unix)]
fn pipe_writer_to_file(writer: &os_pipe::PipeWriter) -> std::fs::File {
  use std::os::unix::io::AsRawFd;
  use std::os::unix::io::FromRawFd;
  unsafe { std::fs::File::from_raw_fd(writer.as_raw_fd()) }
}

/// Creates the stdout and stderr pipes and returns the writers for stdout and stderr.
pub fn create_stdout_stderr_pipes(
  sender: UnboundedSender<BenchEvent>,
) -> (os_pipe::PipeWriter, os_pipe::PipeWriter) {
  let (stdout_reader, stdout_writer) = os_pipe::pipe().unwrap();
  let (stderr_reader, stderr_writer) = os_pipe::pipe().unwrap();

  start_output_redirect_thread(stdout_reader, sender.clone(), |bytes| {
    BenchOutput::Stdout(bytes)
  });
  start_output_redirect_thread(stderr_reader, sender, |bytes| {
    BenchOutput::Stderr(bytes)
  });

  (stdout_writer, stderr_writer)
}

fn start_output_redirect_thread(
  mut pipe_reader: os_pipe::PipeReader,
  sender: UnboundedSender<BenchEvent>,
  map_bench_output: impl Fn(Vec<u8>) -> BenchOutput + Send + 'static,
) {
  tokio::task::spawn_blocking(move || loop {
    let mut buffer = [0; 512];
    let size = match pipe_reader.read(&mut buffer) {
      Ok(0) | Err(_) => break,
      Ok(size) => size,
    };
    if sender
      .send(BenchEvent::Output(map_bench_output(
        buffer[0..size].to_vec(),
      )))
      .is_err()
    {
      break;
    }
  });
}

pub struct Unstable(pub bool);

fn check_unstable(state: &OpState, api_name: &str) {
  let unstable = state.borrow::<Unstable>();

  if !unstable.0 {
    eprintln!(
      "Unstable API '{}'. The --unstable flag must be provided.",
      api_name
    );
    std::process::exit(70);
  }
}

#[op]
fn op_bench_check_unstable(state: &mut OpState) -> Result<(), AnyError> {
  check_unstable(state, "Deno.bench");
  Ok(())
}

#[derive(Clone)]
struct PermissionsHolder(Uuid, Permissions);

#[op]
pub fn op_pledge_test_permissions(
  state: &mut OpState,
  args: ChildPermissionsArg,
) -> Result<Uuid, AnyError> {
  let token = Uuid::new_v4();
  let parent_permissions = state.borrow_mut::<Permissions>();
  let worker_permissions = create_child_permissions(parent_permissions, args)?;
  let parent_permissions = parent_permissions.clone();

  state.put::<PermissionsHolder>(PermissionsHolder(token, parent_permissions));

  // NOTE: This call overrides current permission set for the worker
  state.put::<Permissions>(worker_permissions);

  Ok(token)
}

#[op]
pub fn op_restore_test_permissions(
  state: &mut OpState,
  token: Uuid,
) -> Result<(), AnyError> {
  if let Some(permissions_holder) = state.try_take::<PermissionsHolder>() {
    if token != permissions_holder.0 {
      panic!("restore test permissions token does not match the stored token");
    }

    let permissions = permissions_holder.1;
    state.put::<Permissions>(permissions);
    Ok(())
  } else {
    Err(generic_error("no permissions to restore"))
  }
}

#[op]
fn op_get_bench_origin(state: &mut OpState) -> Result<String, AnyError> {
  Ok(state.borrow::<ModuleSpecifier>().to_string())
}

#[op]
fn op_dispatch_bench_event(
  state: &mut OpState,
  event: BenchEvent,
) -> Result<(), AnyError> {
  let sender = state.borrow::<UnboundedSender<BenchEvent>>().clone();
  sender.send(event).ok();

  Ok(())
}

#[op]
fn op_bench_now(state: &mut OpState) -> Result<u64, AnyError> {
  let ns = state.borrow::<time::Instant>().elapsed().as_nanos();
  let ns_u64 = u64::try_from(ns)?;
  Ok(ns_u64)
}

#[op]
pub fn op_print(
  state: &mut OpState,
  msg: String,
  is_err: bool,
) -> Result<(), AnyError> {
  let sender = state.borrow::<UnboundedSender<BenchEvent>>().clone();
  let msg = if is_err {
    BenchOutput::PrintStderr(msg)
  } else {
    BenchOutput::PrintStdout(msg)
  };
  sender.send(BenchEvent::Output(msg)).ok();
  Ok(())
}
