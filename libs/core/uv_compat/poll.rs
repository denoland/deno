// Copyright 2018-2026 the Deno authors. MIT license.

// Unix uv_poll driver.
//
// Each UvLoopInner lazily creates one worker and joins it during loop drop.
// The worker aggregates all armed descriptors in poll(..., -1), so a control
// pipe is needed to interrupt that otherwise indefinite wait for updates and
// shutdown. Its commands and ready records contain numeric state only; addon
// pointers stay on the loop thread.
//
// An owner groups watches created by one embedding scope, such as an N-API
// Env. During teardown, invalidation makes the loop reject queued readiness
// and new starts, and StopOwner removes its worker watches.
//
// A ready watch remains disarmed while readiness is outstanding. That
// back-pressure prevents a level-ready fd from flooding loop work while other
// handles remain pollable. The loop validates owner and generation before
// dispatch and again before rearming, discarding stale records.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::ffi::c_int;
use std::ffi::c_short;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;

use super::waker::LoopShared;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PollWatch {
  // Stable handle identity. The worker uses this instead of an ABI pointer.
  pub token: u64,
  // Version of this handle's watch. Stop and restart make older work stale.
  pub generation: u64,
  // Embedding scope whose teardown stops all of its watches together.
  pub owner: u64,
  pub fd: c_int,
  pub poll_events: c_short,
}

// Worker-to-loop handoff: the worker has disarmed this token/generation, and
// the loop validates its current handle and owner before the ABI callback.
// `status` is zero for readiness or a negative worker errno; successful
// records carry raw poll(2) `revents` for loop-side libuv translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PollReady {
  pub token: u64,
  pub generation: u64,
  pub status: c_int,
  pub revents: c_short,
}

enum PollCommand {
  Upsert(PollWatch),
  Stop { token: u64, generation: u64 },
  Rearm { token: u64, generation: u64 },
  StopOwner { owner: u64 },
  Shutdown,
}

#[cfg(test)]
type PollFn =
  Arc<dyn Fn(&mut [libc::pollfd], c_int) -> Result<c_int, c_int> + Send + Sync>;

struct PollControlState {
  commands: VecDeque<PollCommand>,
  ready: VecDeque<PollReady>,
  permanent_failure: Option<c_int>,
  shutdown: bool,
  control_read: Option<OwnedFd>,
  control_write: Option<OwnedFd>,
}

pub(crate) struct PollControl {
  state: Mutex<PollControlState>,
  shared: Arc<LoopShared>,
  // Syscall injection is test-only. Production calls `default_poll` directly
  // so each loop avoids an Arc allocation and dynamic dispatch in its worker.
  #[cfg(test)]
  poll: PollFn,
}

struct WorkerWatch {
  watch: PollWatch,
  // Cleared when readiness is queued; matching Rearm after the callback makes
  // the fd pollable again.
  armed: bool,
}

pub(crate) struct PollDriver {
  control: Arc<PollControl>,
  worker: Option<JoinHandle<()>>,
  #[cfg(test)]
  worker_starts: usize,
  #[cfg(test)]
  upsert_hook: Option<PollUpsertHook>,
}

#[cfg(test)]
pub(crate) struct PollUpsertHook {
  entered_sender: std::sync::mpsc::SyncSender<()>,
  release_receiver: std::sync::mpsc::Receiver<()>,
}

#[cfg(test)]
impl PollUpsertHook {
  fn wait(&self) {
    self
      .entered_sender
      .send(())
      .expect("upsert hook entered receiver disconnected");
    self
      .release_receiver
      .recv()
      .expect("upsert hook release sender disconnected");
  }
}

impl PollDriver {
  pub(crate) fn new(shared: Arc<LoopShared>) -> Self {
    Self {
      control: Arc::new(PollControl {
        state: Mutex::new(PollControlState {
          commands: VecDeque::new(),
          ready: VecDeque::new(),
          permanent_failure: None,
          shutdown: false,
          control_read: None,
          control_write: None,
        }),
        shared,
        #[cfg(test)]
        poll: Arc::new(default_poll),
      }),
      worker: None,
      #[cfg(test)]
      worker_starts: 0,
      #[cfg(test)]
      upsert_hook: None,
    }
  }

  #[cfg(test)]
  pub(crate) fn new_with_poll_for_test(
    shared: Arc<LoopShared>,
    poll: PollFn,
  ) -> Self {
    let mut driver = Self::new(shared);
    Arc::get_mut(&mut driver.control)
      .expect("a new poll driver has the only PollControl reference")
      .poll = poll;
    driver
  }

  #[cfg(test)]
  pub(crate) fn new_with_poll_and_upsert_hook_for_test(
    shared: Arc<LoopShared>,
    poll: PollFn,
    upsert_hook: PollUpsertHook,
  ) -> Self {
    let mut driver = Self::new_with_poll_for_test(shared, poll);
    driver.upsert_hook = Some(upsert_hook);
    driver
  }

  pub(crate) fn upsert(
    &mut self,
    watch: PollWatch,
    owner_live: &AtomicBool,
  ) -> Result<(), c_int> {
    self.start_worker()?;
    #[cfg(test)]
    if let Some(hook) = &self.upsert_hook {
      hook.wait();
    }
    self.upsert_command(watch, owner_live)
  }

  pub(crate) fn stop(&self, token: u64, generation: u64) {
    // This is best-effort after terminal failure or shutdown: the worker will
    // not poll addon fds again, and callers separately invalidate the loop-side
    // generation to suppress queued records.
    let _ = self.command(PollCommand::Stop { token, generation });
  }

  pub(crate) fn rearm(&self, token: u64, generation: u64) -> Result<(), c_int> {
    self.command(PollCommand::Rearm { token, generation })
  }

  pub(crate) fn control(&self) -> Weak<PollControl> {
    Arc::downgrade(&self.control)
  }

  pub(crate) fn drain_ready(&self) -> VecDeque<PollReady> {
    std::mem::take(&mut lock_state(&self.control).ready)
  }

  pub(crate) fn shutdown(&mut self) {
    {
      let mut state = lock_state(&self.control);
      if !state.shutdown {
        state.shutdown = true;
        state.permanent_failure.get_or_insert(libc::ECANCELED);
        if self.worker.is_some() {
          state.commands.push_back(PollCommand::Shutdown);
          signal_control(&state);
        }
      }
    }
    if let Some(worker) = self.worker.take() {
      let _ = worker.join();
    }
  }

  fn start_worker(&mut self) -> Result<(), c_int> {
    if self.worker.is_some() {
      let state = lock_state(&self.control);
      return state.permanent_failure.map_or(Ok(()), Err);
    }

    let control_fd = {
      let mut state = lock_state(&self.control);
      if let Some(errno) = state.permanent_failure {
        return Err(errno);
      }
      if state.shutdown {
        return Err(libc::ECANCELED);
      }
      if state.control_read.is_none() {
        let (read, write) = control_pipe()?;
        state.control_read = Some(read);
        state.control_write = Some(write);
      }
      state
        .control_read
        .as_ref()
        .expect("control pipe must be initialized")
        .as_raw_fd()
    };

    // `control_fd` borrows `state.control_read`. The `Arc<PollControl>` moved
    // into the worker keeps the `OwnedFd` alive; `shutdown` joins it first.
    let control = self.control.clone();
    match std::thread::Builder::new()
      .name("uv-poll".to_owned())
      .spawn(move || worker_loop(control, control_fd))
    {
      Ok(worker) => {
        self.worker = Some(worker);
        #[cfg(test)]
        {
          self.worker_starts += 1;
        }
        Ok(())
      }
      Err(err) => {
        let errno = err.raw_os_error().unwrap_or(libc::EAGAIN);
        let mut state = lock_state(&self.control);
        state.permanent_failure = Some(errno);
        state.control_read = None;
        state.control_write = None;
        Err(errno)
      }
    }
  }

  fn command(&self, command: PollCommand) -> Result<(), c_int> {
    let mut state = lock_state(&self.control);
    if let Some(errno) = state.permanent_failure {
      return Err(errno);
    }
    if state.shutdown {
      return Err(libc::ECANCELED);
    }
    state.commands.push_back(command);
    signal_control(&state);
    Ok(())
  }

  fn upsert_command(
    &self,
    watch: PollWatch,
    owner_live: &AtomicBool,
  ) -> Result<(), c_int> {
    let mut state = lock_state(&self.control);
    if let Some(errno) = state.permanent_failure {
      return Err(errno);
    }
    if state.shutdown {
      return Err(libc::ECANCELED);
    }
    // The command mutex makes either ordering safe: invalidation follows an
    // already queued upsert with StopOwner, while a later upsert sees false.
    if !owner_live.load(Ordering::Acquire) {
      return Err(libc::ECANCELED);
    }
    state.commands.push_back(PollCommand::Upsert(watch));
    signal_control(&state);
    Ok(())
  }
}

impl PollControl {
  pub(crate) fn stop_owner(&self, owner: u64) {
    let mut state = lock_state(self);
    if !state.shutdown && state.permanent_failure.is_none() {
      state.commands.push_back(PollCommand::StopOwner { owner });
      signal_control(&state);
    }
    // Owner invalidation can race with a terminal worker error. Wake the loop
    // even when no command can be queued so it observes the invalidation.
    self.shared.loop_waker.wake();
  }
}

impl Drop for PollDriver {
  fn drop(&mut self) {
    self.shutdown();
  }
}

fn control_pipe() -> Result<(OwnedFd, OwnedFd), c_int> {
  let mut fds = [0; 2];
  #[cfg(target_os = "linux")]
  {
    // Atomically set CLOEXEC to avoid a concurrent fork/exec inheritance race.
    if unsafe {
      libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK)
    } == -1
    {
      return Err(last_errno());
    }
    let read = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let write = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    Ok((read, write))
  }
  #[cfg(not(target_os = "linux"))]
  {
    if unsafe { libc::pipe(fds.as_mut_ptr()) } == -1 {
      return Err(last_errno());
    }
    let read = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let write = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    for fd in fds {
      set_fd_nonblocking(fd)?;
      let flags = fcntl_retry(fd, libc::F_GETFD, 0)?;
      fcntl_retry(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC)?;
    }
    Ok((read, write))
  }
}

pub(crate) fn set_fd_nonblocking(fd: c_int) -> Result<(), c_int> {
  let flags = fcntl_retry(fd, libc::F_GETFL, 0)?;
  if flags & libc::O_NONBLOCK == 0 {
    fcntl_retry(fd, libc::F_SETFL, flags | libc::O_NONBLOCK)?;
  }
  Ok(())
}

pub(crate) fn uv_events_to_poll_events(events: c_int) -> c_short {
  let mut poll_events = 0;
  if events & super::UV_READABLE != 0 {
    poll_events |= libc::POLLIN;
  }
  if events & super::UV_WRITABLE != 0 {
    poll_events |= libc::POLLOUT;
  }
  if events & super::UV_PRIORITIZED != 0 {
    poll_events |= libc::POLLPRI;
  }
  // POLLRDHUP is only available on these targets. Elsewhere, omitting
  // UV_DISCONNECT is fine: libuv treats it as an optional shutdown
  // optimization.
  #[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "illumos",
  ))]
  if events & super::UV_DISCONNECT != 0 {
    poll_events |= libc::POLLRDHUP;
  }
  poll_events
}

pub(crate) fn poll_revents_to_uv_callback_args(
  revents: c_short,
  requested_events: c_int,
) -> (c_int, c_int) {
  let mut cb_events = 0;
  if revents & libc::POLLIN != 0 {
    cb_events |= super::UV_READABLE;
  }
  if revents & libc::POLLOUT != 0 {
    cb_events |= super::UV_WRITABLE;
  }
  if revents & libc::POLLPRI != 0 {
    cb_events |= super::UV_PRIORITIZED;
  }
  #[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "illumos",
  ))]
  if revents & libc::POLLRDHUP != 0 {
    cb_events |= super::UV_DISCONNECT;
  }

  // Closing an active poll fd is invalid libuv usage. When poll(2) detects it
  // as POLLNVAL, report terminal UV_EBADF so loop dispatch stops the handle
  // and releases its raw-fd ownership. This cleanup is best-effort, however:
  // fd-number reuse can occur before POLLNVAL is observed, in which case the
  // worker cannot distinguish the replacement descriptor from the original.
  if revents & libc::POLLNVAL != 0 {
    return (super::UV_EBADF, 0);
  }
  // libuv preserves POLLERR | POLLPRI as prioritized readiness. Linux and
  // FreeBSD sysfs/kernfs use that combination, so it is not an EBADF error.
  if revents & libc::POLLERR != 0 && revents & libc::POLLPRI == 0 {
    return (super::UV_EBADF, 0);
  }
  // On error or hangup, libuv mixes in the requested interests so the
  // appropriate read or write callback can run even without a readiness bit.
  if revents & (libc::POLLERR | libc::POLLHUP) != 0 {
    cb_events |= requested_events
      & (super::UV_READABLE
        | super::UV_WRITABLE
        | super::UV_DISCONNECT
        | super::UV_PRIORITIZED);
  }
  (0, cb_events)
}

fn fcntl_retry(
  fd: c_int,
  command: c_int,
  argument: c_int,
) -> Result<c_int, c_int> {
  loop {
    // Getter commands such as F_GETFL ignore the variadic third argument,
    // so passing a placeholder value such as 0 is safe.
    let result = unsafe { libc::fcntl(fd, command, argument) };
    if result != -1 {
      return Ok(result);
    }
    let errno = last_errno();
    if errno != libc::EINTR {
      return Err(errno);
    }
  }
}

fn worker_loop(control: Arc<PollControl>, control_fd: c_int) {
  let mut watches: HashMap<u64, WorkerWatch> = HashMap::new();
  loop {
    let mut fds = vec![libc::pollfd {
      fd: control_fd,
      events: libc::POLLIN,
      revents: 0,
    }];
    let mut snapshot = Vec::new();
    for worker_watch in watches.values() {
      if worker_watch.armed {
        fds.push(libc::pollfd {
          fd: worker_watch.watch.fd,
          events: worker_watch.watch.poll_events,
          revents: 0,
        });
        snapshot
          .push((worker_watch.watch.token, worker_watch.watch.generation));
      }
    }

    #[cfg(test)]
    let poll_result = (control.poll)(&mut fds, -1);
    #[cfg(not(test))]
    let poll_result = default_poll(&mut fds, -1);

    match poll_result {
      Ok(num_ready_fds) => {
        let mut remaining = num_ready_fds as usize;
        if fds[0].revents != 0 {
          remaining -= 1;
          drain_control_fd(control_fd);
          // Commands may replace or stop watches captured by this poll, so
          // apply them first. Generation and armed checks prevent an old
          // snapshot from disarming a replacement or queuing readiness for a
          // stopped watch.
          if process_commands(&control, &mut watches) {
            return;
          }
        }

        let mut ready = Vec::new();
        for (index, (token, generation)) in snapshot.into_iter().enumerate() {
          if remaining == 0 {
            break;
          }
          let revents = fds[index + 1].revents;
          if revents == 0 {
            continue;
          }
          remaining -= 1;
          if let Some(worker_watch) = watches.get_mut(&token)
            && worker_watch.armed
            && worker_watch.watch.generation == generation
          {
            worker_watch.armed = false;
            ready.push(PollReady {
              token,
              generation,
              status: 0,
              revents,
            });
          }
        }
        push_ready(&control, ready);
      }
      Err(errno) => {
        drain_control_fd(control_fd);
        let commands = fail_and_take_commands(&control, errno);
        if process_command_queue(commands, &mut watches) {
          return;
        }
        fail_watches(&control, &mut watches, errno);
        service_shutdown(&control, control_fd, &mut watches);
        return;
      }
    }
  }
}

fn process_commands(
  control: &PollControl,
  watches: &mut HashMap<u64, WorkerWatch>,
) -> bool {
  let commands = {
    let mut state = lock_state(control);
    std::mem::take(&mut state.commands)
  };
  process_command_queue(commands, watches)
}

fn process_command_queue(
  commands: VecDeque<PollCommand>,
  watches: &mut HashMap<u64, WorkerWatch>,
) -> bool {
  for command in commands {
    match command {
      PollCommand::Upsert(watch) => {
        watches.insert(watch.token, WorkerWatch { watch, armed: true });
      }
      PollCommand::Stop { token, generation } => {
        if watches
          .get(&token)
          .is_some_and(|watch| watch.watch.generation == generation)
        {
          watches.remove(&token);
        }
      }
      PollCommand::Rearm { token, generation } => {
        if let Some(watch) = watches.get_mut(&token)
          && watch.watch.generation == generation
        {
          watch.armed = true;
        }
      }
      PollCommand::StopOwner { owner } => {
        watches.retain(|_, watch| watch.watch.owner != owner)
      }
      PollCommand::Shutdown => return true,
    }
  }
  false
}

fn fail_and_take_commands(
  control: &PollControl,
  errno: c_int,
) -> VecDeque<PollCommand> {
  let mut state = lock_state(control);
  // An aggregate poll failure cannot be attributed to one watch, so the worker
  // becomes terminal. Commands accepted before it apply first: stops suppress
  // errors, upserts join the one-time fanout, and Shutdown exits immediately.
  state.permanent_failure.get_or_insert(errno);
  std::mem::take(&mut state.commands)
}

fn fail_watches(
  control: &PollControl,
  watches: &mut HashMap<u64, WorkerWatch>,
  errno: c_int,
) {
  // Only armed watches need an error record. A disarmed watch has queued
  // readiness; if it remains current through dispatch, Rearm sees
  // permanent_failure and the loop stops the handle.
  let mut ready = Vec::new();
  for watch in watches.values_mut() {
    if watch.armed {
      watch.armed = false;
      ready.push(PollReady {
        token: watch.watch.token,
        generation: watch.watch.generation,
        status: -errno,
        revents: 0,
      });
    }
  }
  push_ready(control, ready);
}

fn service_shutdown(
  control: &PollControl,
  control_fd: c_int,
  watches: &mut HashMap<u64, WorkerWatch>,
) {
  // A terminal poll failure has already reported each armed watch. From here
  // service only the control pipe until Shutdown arrives, rather than touching
  // addon fds again or leaving a worker that loop drop cannot join.
  loop {
    let mut fd = libc::pollfd {
      fd: control_fd,
      events: libc::POLLIN,
      revents: 0,
    };
    match default_poll(std::slice::from_mut(&mut fd), -1) {
      Ok(_) => {
        drain_control_fd(control_fd);
        if process_commands(control, watches) {
          return;
        }
      }
      Err(_) => return,
    }
  }
}

fn push_ready(control: &PollControl, ready: Vec<PollReady>) {
  if ready.is_empty() {
    return;
  }
  lock_state(control).ready.extend(ready);
  control.shared.loop_waker.wake();
}

fn drain_control_fd(fd: c_int) {
  let mut bytes = [0_u8; 64];
  loop {
    let result =
      unsafe { libc::read(fd, bytes.as_mut_ptr().cast(), bytes.len()) };
    if result > 0 {
      continue;
    }
    if result == -1 && last_errno() == libc::EINTR {
      continue;
    }
    return;
  }
}

fn signal_control(state: &PollControlState) {
  let Some(fd) = state.control_write.as_ref().map(AsRawFd::as_raw_fd) else {
    return;
  };
  loop {
    let result = unsafe { libc::write(fd, b"x".as_ptr().cast(), 1) };
    if result == 1 {
      return;
    }
    if result == -1 {
      let errno = last_errno();
      if errno == libc::EINTR {
        continue;
      }
      if errno == libc::EAGAIN || errno == libc::EWOULDBLOCK {
        // A full nonblocking pipe already holds a wakeup byte. Dropping this
        // byte deliberately coalesces updates; the worker drains the pipe and
        // then takes the whole command queue under the same mutex.
        return;
      }
    }
    return;
  }
}

fn default_poll(
  fds: &mut [libc::pollfd],
  timeout: c_int,
) -> Result<c_int, c_int> {
  loop {
    let result = unsafe {
      libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout)
    };
    if result >= 0 {
      return Ok(result);
    }
    let errno = last_errno();
    if errno != libc::EINTR {
      return Err(errno);
    }
  }
}

fn last_errno() -> c_int {
  std::io::Error::last_os_error()
    .raw_os_error()
    .unwrap_or(libc::EIO)
}

fn lock_state(control: &PollControl) -> MutexGuard<'_, PollControlState> {
  control
    .state
    .lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
  // Pure translation tests pin libuv mask semantics. Driver tests use injection
  // for state-machine and concurrency edges; end-to-end addon coverage lives
  // in tests/napi.
  use std::cell::Cell;
  use std::future::poll_fn;
  use std::os::fd::AsRawFd;
  use std::os::fd::FromRawFd;
  use std::os::fd::OwnedFd;
  use std::sync::Arc;
  use std::sync::Barrier;
  use std::sync::atomic::AtomicBool;
  use std::sync::atomic::AtomicUsize;
  use std::sync::atomic::Ordering;
  use std::sync::mpsc;
  use std::task::Context;
  use std::task::Wake;
  use std::task::Waker;
  use std::time::Duration;
  use std::time::Instant;

  use super::*;
  use crate::JsRuntime;
  use crate::PollEventLoopOptions;
  use crate::uv_compat::waker::LoopShared;
  use crate::uv_compat::*;

  const DEADLINE: Duration = Duration::from_secs(5);
  static OWNER_LIVE: AtomicBool = AtomicBool::new(true);

  #[test]
  fn uv_events_translate_to_poll_events() {
    assert_eq!(
      uv_events_to_poll_events(UV_READABLE | UV_WRITABLE | UV_PRIORITIZED),
      libc::POLLIN | libc::POLLOUT | libc::POLLPRI,
    );

    #[cfg(any(
      target_os = "linux",
      target_os = "android",
      target_os = "freebsd",
      target_os = "illumos",
    ))]
    assert_eq!(uv_events_to_poll_events(UV_DISCONNECT), libc::POLLRDHUP);

    #[cfg(not(any(
      target_os = "linux",
      target_os = "android",
      target_os = "freebsd",
      target_os = "illumos",
    )))]
    assert_eq!(uv_events_to_poll_events(UV_DISCONNECT), 0);
  }

  #[test]
  fn poll_revents_translate_to_uv_callback_args() {
    assert_eq!(
      poll_revents_to_uv_callback_args(
        libc::POLLIN | libc::POLLOUT | libc::POLLPRI,
        UV_READABLE | UV_WRITABLE | UV_DISCONNECT | UV_PRIORITIZED,
      ),
      (0, UV_READABLE | UV_WRITABLE | UV_PRIORITIZED),
    );
    assert_eq!(
      poll_revents_to_uv_callback_args(libc::POLLERR, UV_READABLE),
      (UV_EBADF, 0),
    );
    assert_eq!(
      poll_revents_to_uv_callback_args(
        libc::POLLERR | libc::POLLPRI,
        UV_PRIORITIZED,
      ),
      (0, UV_PRIORITIZED),
    );
    assert_eq!(
      poll_revents_to_uv_callback_args(
        libc::POLLHUP,
        UV_READABLE | UV_WRITABLE | UV_PRIORITIZED | UV_DISCONNECT,
      ),
      (
        0,
        UV_READABLE | UV_WRITABLE | UV_PRIORITIZED | UV_DISCONNECT,
      ),
    );
    assert_eq!(
      poll_revents_to_uv_callback_args(libc::POLLNVAL, UV_READABLE),
      (UV_EBADF, 0),
    );

    #[cfg(any(
      target_os = "linux",
      target_os = "android",
      target_os = "freebsd",
      target_os = "illumos",
    ))]
    assert_eq!(
      poll_revents_to_uv_callback_args(libc::POLLRDHUP, UV_READABLE),
      (0, UV_DISCONNECT),
    );
  }

  fn pipe() -> (OwnedFd, OwnedFd) {
    let mut fds = [0; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
  }

  fn watch(token: u64, fd: c_int) -> PollWatch {
    watch_with_generation(token, 1, fd)
  }

  fn watch_with_generation(
    token: u64,
    generation: u64,
    fd: c_int,
  ) -> PollWatch {
    PollWatch {
      token,
      generation,
      owner: 999,
      fd,
      poll_events: libc::POLLIN,
    }
  }

  fn write_byte(fd: &OwnedFd) {
    assert_eq!(
      unsafe { libc::write(fd.as_raw_fd(), b"x".as_ptr().cast(), 1) },
      1
    );
  }

  fn wait_for_ready(driver: &PollDriver, count: usize) -> Vec<PollReady> {
    let deadline = Instant::now() + DEADLINE;
    let mut ready = Vec::new();
    while ready.len() < count {
      ready.extend(driver.drain_ready());
      if ready.len() >= count {
        return ready;
      }
      assert!(
        Instant::now() < deadline,
        "poll readiness did not arrive within {DEADLINE:?}"
      );
      std::thread::yield_now();
    }
    ready
  }

  async fn tick(runtime: &mut JsRuntime) {
    poll_fn(|cx| {
      let _ = runtime.poll_event_loop(cx, PollEventLoopOptions::default());
      std::task::Poll::Ready(())
    })
    .await;
  }

  async fn wait_for_queued_ready(loop_: *mut uv_loop_t) {
    wait_for_queued_ready_count(loop_, 1).await;
  }

  async fn wait_for_queued_ready_count(loop_: *mut uv_loop_t, count: usize) {
    let deadline = Instant::now() + DEADLINE;
    loop {
      let ready_count = unsafe {
        let driver = super::super::get_inner(loop_).poll_driver.borrow();
        lock_state(&driver.control).ready.len()
      };
      if ready_count >= count {
        return;
      }
      assert!(
        Instant::now() < deadline,
        "poll readiness did not queue within {DEADLINE:?}"
      );
      tokio::task::yield_now().await;
    }
  }

  struct PollCallbackState {
    calls: Cell<usize>,
  }

  struct FailureCallbackState {
    calls: Cell<usize>,
    status: Cell<c_int>,
    stopped: Cell<bool>,
    loop_: *mut uv_loop_t,
    release_failure: mpsc::SyncSender<()>,
  }

  struct WakeCounter(Arc<AtomicUsize>);

  impl Wake for WakeCounter {
    fn wake(self: Arc<Self>) {
      self.0.fetch_add(1, Ordering::SeqCst);
    }

    fn wake_by_ref(self: &Arc<Self>) {
      self.0.fetch_add(1, Ordering::SeqCst);
    }
  }

  unsafe extern "C" fn count_callback(
    handle: *mut uv_poll_t,
    _: c_int,
    _: c_int,
  ) {
    // SAFETY: Tests install a valid PollCallbackState in handle.data.
    let state = unsafe { &*((*handle).data as *const PollCallbackState) };
    state.calls.set(state.calls.get() + 1);
  }

  unsafe extern "C" fn noop_callback(_: *mut uv_poll_t, _: c_int, _: c_int) {}

  unsafe extern "C" fn poll_stopped_callback(handle: *mut uv_poll_t) {
    let state = unsafe { &*((*handle).data as *const FailureCallbackState) };
    state.stopped.set(true);
  }

  unsafe extern "C" fn failure_callback(
    handle: *mut uv_poll_t,
    status: c_int,
    _: c_int,
  ) {
    // SAFETY: Tests install a valid FailureCallbackState in handle.data.
    let state = unsafe { &*((*handle).data as *const FailureCallbackState) };
    state.calls.set(state.calls.get() + 1);
    state.status.set(status);
    state
      .release_failure
      .send(())
      .expect("worker did not wait for callback completion");

    let deadline = Instant::now() + DEADLINE;
    while unsafe {
      let driver = super::super::get_inner(state.loop_).poll_driver.borrow();
      lock_state(&driver.control).permanent_failure.is_none()
    } {
      assert!(
        Instant::now() < deadline,
        "worker did not record its permanent failure within {DEADLINE:?}"
      );
      std::thread::yield_now();
    }
  }

  fn runtime_loop(runtime: &JsRuntime) -> *mut uv_loop_t {
    runtime
      .uv_loop_ptr()
      .expect("JsRuntime should have a uv loop")
  }

  #[cfg(not(miri))] // needs I/O
  #[tokio::test(flavor = "current_thread")]
  async fn poll_callback_back_pressure_is_handle_local() {
    let (first_read, first_write) = pipe();
    let (second_read, second_write) = pipe();
    let mut runtime = JsRuntime::new(Default::default());
    let loop_ = runtime_loop(&runtime);
    let first_state = Box::new(PollCallbackState {
      calls: Cell::new(0),
    });
    let second_state = Box::new(PollCallbackState {
      calls: Cell::new(0),
    });
    let mut first_handle = std::mem::MaybeUninit::<uv_poll_t>::uninit();
    let mut second_handle = std::mem::MaybeUninit::<uv_poll_t>::uninit();
    let owner = unsafe { new_poll_owner(loop_) };
    unsafe {
      assert_eq!(
        uv_poll_init(
          loop_,
          first_handle.as_mut_ptr(),
          first_read.as_raw_fd(),
          owner.clone(),
        ),
        0
      );
      assert_eq!(
        uv_poll_init(
          loop_,
          second_handle.as_mut_ptr(),
          second_read.as_raw_fd(),
          owner,
        ),
        0
      );
      (*first_handle.as_mut_ptr()).data = (&*first_state
        as *const PollCallbackState)
        .cast_mut()
        .cast();
      (*second_handle.as_mut_ptr()).data = (&*second_state
        as *const PollCallbackState)
        .cast_mut()
        .cast();
      assert_eq!(
        uv_poll_start(
          first_handle.as_mut_ptr(),
          UV_READABLE,
          Some(count_callback)
        ),
        0
      );
      assert_eq!(
        uv_poll_start(
          second_handle.as_mut_ptr(),
          UV_READABLE,
          Some(count_callback)
        ),
        0
      );
    }
    write_byte(&first_write);
    wait_for_queued_ready(loop_).await;
    write_byte(&second_write);
    wait_for_queued_ready_count(loop_, 2).await;
    let deadline = Instant::now() + DEADLINE;
    while second_state.calls.get() != 1 {
      tick(&mut runtime).await;
      assert!(
        Instant::now() < deadline,
        "poll callback did not arrive within {DEADLINE:?}"
      );
      tokio::task::yield_now().await;
    }
    assert_eq!(first_state.calls.get(), 1);
    assert_eq!(second_state.calls.get(), 1);
    unsafe {
      uv_poll_close(first_handle.as_mut_ptr());
      uv_poll_close(second_handle.as_mut_ptr());
    };
  }

  #[cfg(not(miri))] // needs I/O
  #[tokio::test(flavor = "current_thread")]
  async fn poll_restart_invalidates_queued_generation() {
    let (read_fd, write_fd) = pipe();
    let mut runtime = JsRuntime::new(Default::default());
    let loop_ = runtime_loop(&runtime);
    let state = Box::new(PollCallbackState {
      calls: Cell::new(0),
    });
    let mut handle = std::mem::MaybeUninit::<uv_poll_t>::uninit();
    unsafe {
      assert_eq!(
        uv_poll_init(
          loop_,
          handle.as_mut_ptr(),
          read_fd.as_raw_fd(),
          new_poll_owner(loop_)
        ),
        0
      );
      (*handle.as_mut_ptr()).data =
        (&*state as *const PollCallbackState).cast_mut().cast();
      assert_eq!(
        uv_poll_start(handle.as_mut_ptr(), UV_READABLE, Some(count_callback)),
        0
      );
    }
    write_byte(&write_fd);
    wait_for_queued_ready(loop_).await;
    unsafe {
      assert_eq!(
        uv_poll_start(handle.as_mut_ptr(), UV_WRITABLE, Some(count_callback)),
        0
      );
    }
    tick(&mut runtime).await;
    assert_eq!(state.calls.get(), 0);
    unsafe { uv_poll_close(handle.as_mut_ptr()) };
  }

  #[cfg(not(miri))] // needs I/O
  #[tokio::test(flavor = "current_thread")]
  async fn poll_stop_suppresses_queued_callback() {
    let (read_fd, write_fd) = pipe();
    let mut runtime = JsRuntime::new(Default::default());
    let loop_ = runtime_loop(&runtime);
    let state = Box::new(PollCallbackState {
      calls: Cell::new(0),
    });
    let mut handle = std::mem::MaybeUninit::<uv_poll_t>::uninit();
    unsafe {
      assert_eq!(
        uv_poll_init(
          loop_,
          handle.as_mut_ptr(),
          read_fd.as_raw_fd(),
          new_poll_owner(loop_)
        ),
        0
      );
      (*handle.as_mut_ptr()).data =
        (&*state as *const PollCallbackState).cast_mut().cast();
      assert_eq!(
        uv_poll_start(handle.as_mut_ptr(), UV_READABLE, Some(count_callback)),
        0
      );
    }
    write_byte(&write_fd);
    wait_for_queued_ready(loop_).await;
    unsafe {
      assert_eq!(uv_poll_stop(handle.as_mut_ptr()), 0);
    }
    tick(&mut runtime).await;
    assert_eq!(state.calls.get(), 0);

    unsafe { uv_poll_close(handle.as_mut_ptr()) };
  }

  #[cfg(not(miri))] // needs I/O
  #[tokio::test(flavor = "current_thread")]
  async fn poll_owner_invalidation_suppresses_queued_callback() {
    let (read_fd, write_fd) = pipe();
    let mut runtime = JsRuntime::new(Default::default());
    let loop_ = runtime_loop(&runtime);
    let owner = unsafe { new_poll_owner(loop_) };
    let state = Box::new(PollCallbackState {
      calls: Cell::new(0),
    });
    let mut handle = std::mem::MaybeUninit::<uv_poll_t>::uninit();
    unsafe {
      assert_eq!(
        uv_poll_init(
          loop_,
          handle.as_mut_ptr(),
          read_fd.as_raw_fd(),
          owner.clone()
        ),
        0
      );
      (*handle.as_mut_ptr()).data =
        (&*state as *const PollCallbackState).cast_mut().cast();
      assert_eq!(
        uv_poll_start(handle.as_mut_ptr(), UV_READABLE, Some(count_callback)),
        0
      );
    }
    write_byte(&write_fd);
    wait_for_queued_ready(loop_).await;

    owner.invalidate();
    tick(&mut runtime).await;
    assert_eq!(state.calls.get(), 0);

    unsafe { uv_poll_close(handle.as_mut_ptr()) };
  }

  #[cfg(not(miri))] // needs I/O
  #[tokio::test(flavor = "current_thread")]
  async fn poll_owner_invalidation_wakes_a_pending_loop() {
    let (read_fd, _write_fd) = pipe();
    let mut runtime = JsRuntime::new(Default::default());
    let loop_ = runtime_loop(&runtime);
    let owner = unsafe { new_poll_owner(loop_) };
    let mut handle = std::mem::MaybeUninit::<uv_poll_t>::uninit();
    unsafe {
      assert_eq!(
        uv_poll_init(
          loop_,
          handle.as_mut_ptr(),
          read_fd.as_raw_fd(),
          owner.clone()
        ),
        0
      );
      assert_eq!(
        uv_poll_start(handle.as_mut_ptr(), UV_READABLE, Some(noop_callback)),
        0
      );
    }

    let wake_count = Arc::new(AtomicUsize::new(0));
    let waker = Waker::from(Arc::new(WakeCounter(wake_count.clone())));
    let mut cx = Context::from_waker(&waker);
    assert!(
      runtime
        .poll_event_loop(&mut cx, PollEventLoopOptions::default())
        .is_pending(),
      "event loop should be pending before owner invalidation"
    );
    wake_count.store(0, Ordering::SeqCst);

    std::thread::spawn(move || owner.invalidate())
      .join()
      .expect("owner invalidation thread panicked");

    assert_eq!(wake_count.load(Ordering::SeqCst), 1);
    assert!(
      runtime
        .poll_event_loop(&mut cx, PollEventLoopOptions::default())
        .is_ready(),
      "invalidated owner must not keep a refed poll handle alive"
    );
    unsafe { uv_poll_close(handle.as_mut_ptr()) };
  }

  #[cfg(not(miri))] // needs I/O
  #[tokio::test(flavor = "current_thread")]
  async fn poll_start_rejects_owner_invalidated_at_upsert_boundary() {
    let (read_fd, _write_fd) = pipe();
    let runtime = JsRuntime::new(Default::default());
    let loop_ = runtime_loop(&runtime);
    let (entered_sender, entered_receiver) = mpsc::sync_channel(0);
    let (release_sender, release_receiver) = mpsc::sync_channel(0);
    let inner = unsafe { super::super::get_inner(loop_) };
    *inner.poll_driver.borrow_mut() =
      PollDriver::new_with_poll_and_upsert_hook_for_test(
        inner.shared.clone(),
        Arc::new(default_poll),
        PollUpsertHook {
          entered_sender,
          release_receiver,
        },
      );

    let owner = unsafe { new_poll_owner(loop_) };
    let owner_for_invalidator = owner.clone();
    let invalidator = std::thread::spawn(move || {
      entered_receiver
        .recv_timeout(DEADLINE)
        .expect("start did not reach the upsert boundary");
      owner_for_invalidator.invalidate();
      release_sender
        .send(())
        .expect("start did not wait for the invalidation");
    });
    let mut handle = std::mem::MaybeUninit::<uv_poll_t>::uninit();
    unsafe {
      assert_eq!(
        uv_poll_init(loop_, handle.as_mut_ptr(), read_fd.as_raw_fd(), owner),
        0
      );
      assert_eq!(
        uv_poll_start(handle.as_mut_ptr(), UV_READABLE, Some(noop_callback)),
        UV_ECANCELED
      );
      assert_eq!(uv_is_active(handle.as_ptr().cast()), 0);
      assert!(
        !inner
          .fd_watchers
          .borrow()
          .contains_key(&read_fd.as_raw_fd()),
        "rejected start must not claim fd ownership"
      );
      uv_poll_close(handle.as_mut_ptr());
    }
    invalidator.join().expect("invalidation thread panicked");
  }

  #[cfg(not(miri))] // needs I/O
  #[tokio::test(flavor = "current_thread")]
  async fn poll_permanent_failure_after_callback_releases_pending_handle() {
    let (read_fd, _write_fd) = pipe();
    let mut runtime = JsRuntime::new(Default::default());
    let loop_ = runtime_loop(&runtime);
    let (release_failure, failure_release) = mpsc::sync_channel(0);
    let ready_once = Arc::new(AtomicBool::new(false));
    let ready_once_for_worker = ready_once.clone();
    let failure_release_for_worker = Arc::new(Mutex::new(failure_release));
    let failure_release_for_worker = failure_release_for_worker.clone();
    let inner = unsafe { super::super::get_inner(loop_) };
    *inner.poll_driver.borrow_mut() = PollDriver::new_with_poll_for_test(
      inner.shared.clone(),
      Arc::new(move |fds, timeout| {
        if fds.len() > 1 && !ready_once_for_worker.swap(true, Ordering::SeqCst)
        {
          fds[1].revents = libc::POLLIN;
          return Ok(1);
        }
        if ready_once_for_worker.load(Ordering::SeqCst) {
          failure_release_for_worker
            .lock()
            .unwrap()
            .recv_timeout(DEADLINE)
            .expect("callback did not release permanent failure");
          return Err(libc::EIO);
        }
        default_poll(fds, timeout)
      }),
    );

    let state = Box::new(FailureCallbackState {
      calls: Cell::new(0),
      status: Cell::new(-1),
      stopped: Cell::new(false),
      loop_,
      release_failure,
    });
    let mut handle = std::mem::MaybeUninit::<uv_poll_t>::uninit();
    unsafe {
      assert_eq!(
        uv_poll_init(
          loop_,
          handle.as_mut_ptr(),
          read_fd.as_raw_fd(),
          new_poll_owner(loop_)
        ),
        0
      );
      (*handle.as_mut_ptr()).data =
        (&*state as *const FailureCallbackState).cast_mut().cast();
      uv_poll_set_stop_callback(
        handle.as_mut_ptr(),
        Some(poll_stopped_callback),
      );
      assert_eq!(
        uv_poll_start(handle.as_mut_ptr(), UV_READABLE, Some(failure_callback)),
        0
      );
    }
    wait_for_queued_ready(loop_).await;
    tick(&mut runtime).await;

    assert_eq!(state.calls.get(), 1);
    assert_eq!(state.status.get(), 0);
    assert!(
      state.stopped.get(),
      "terminal rearm failure must notify bridges"
    );
    assert_eq!(unsafe { uv_is_active(handle.as_ptr().cast()) }, 0);
    assert!(
      !inner
        .fd_watchers
        .borrow()
        .contains_key(&read_fd.as_raw_fd()),
      "failed rearm must release fd ownership"
    );
    tick(&mut runtime).await;
    assert_eq!(state.calls.get(), 1, "failure must not redeliver callback");
    unsafe { uv_poll_close(handle.as_mut_ptr()) };
  }

  #[cfg(not(miri))] // needs I/O
  #[test]
  fn poll_owner_invalidation_does_not_blacklist_reused_id() {
    let (first_read, _first_write) = pipe();
    let (second_read, _second_write) = pipe();
    let shared = LoopShared::new();
    let mut driver = PollDriver::new(shared);
    let first_owner_live = AtomicBool::new(true);
    let second_owner_live = AtomicBool::new(true);

    driver
      .upsert(
        PollWatch {
          token: 1,
          generation: 1,
          owner: 7,
          fd: first_read.as_raw_fd(),
          events: libc::POLLIN,
        },
        &first_owner_live,
      )
      .unwrap();
    driver.control.stop_owner(7);

    assert!(
      driver
        .upsert(
          PollWatch {
            token: 2,
            generation: 1,
            owner: 7,
            fd: second_read.as_raw_fd(),
            events: libc::POLLIN,
          },
          &second_owner_live,
        )
        .is_ok()
    );

    driver.shutdown();
  }

  #[cfg(not(miri))] // needs I/O
  #[test]
  fn one_worker_for_many_handles() {
    const HANDLE_COUNT: usize = 64;

    let runtime = JsRuntime::new(Default::default());
    let loop_ = runtime_loop(&runtime);
    let mut pipes = Vec::with_capacity(HANDLE_COUNT);
    let mut handles = Vec::with_capacity(HANDLE_COUNT);
    let owner = unsafe { new_poll_owner(loop_) };
    for _ in 0..HANDLE_COUNT {
      pipes.push(pipe());
      handles.push(std::mem::MaybeUninit::<uv_poll_t>::uninit());
    }

    for ((read_fd, _write_fd), handle) in pipes.iter().zip(handles.iter_mut()) {
      unsafe {
        assert_eq!(
          uv_poll_init(
            loop_,
            handle.as_mut_ptr(),
            read_fd.as_raw_fd(),
            owner.clone(),
          ),
          0
        );
        assert_eq!(
          uv_poll_start(handle.as_mut_ptr(), UV_READABLE, Some(noop_callback)),
          0
        );
      }
    }

    let worker_starts = unsafe {
      super::super::get_inner(loop_)
        .poll_driver
        .borrow()
        .worker_starts
    };
    assert_eq!(worker_starts, 1);

    for handle in &mut handles {
      unsafe {
        assert_eq!(uv_poll_stop(handle.as_mut_ptr()), 0);
        uv_poll_close(handle.as_mut_ptr());
      }
    }
    drop(runtime);
  }

  #[cfg(not(miri))] // needs I/O
  #[test]
  fn control_interrupts_indefinite_poll() {
    let (read_a, _write_a) = pipe();
    let (read_b, write_b) = pipe();
    let (entered_second_poll, second_poll_entered) = mpsc::sync_channel(0);
    let poll_calls = Arc::new(AtomicUsize::new(0));
    let poll_calls_for_worker = poll_calls.clone();
    let shared = LoopShared::new();
    let mut driver = PollDriver::new_with_poll_for_test(
      shared,
      Arc::new(move |fds, timeout| {
        if poll_calls_for_worker.fetch_add(1, Ordering::SeqCst) == 1 {
          let _ = entered_second_poll.send(());
        }
        default_poll(fds, timeout)
      }),
    );

    driver
      .upsert(watch(1, read_a.as_raw_fd()), &OWNER_LIVE)
      .unwrap();
    second_poll_entered
      .recv_timeout(DEADLINE)
      .expect("worker did not begin its indefinite poll within {DEADLINE:?}");
    driver
      .upsert(watch(2, read_b.as_raw_fd()), &OWNER_LIVE)
      .unwrap();
    write_byte(&write_b);

    assert_eq!(wait_for_ready(&driver, 1)[0].token, 2);
    driver.shutdown();
  }

  #[cfg(not(miri))] // needs I/O
  #[test]
  fn ready_watch_waits_for_rearm() {
    let (read_fd, write_fd) = pipe();
    let shared = LoopShared::new();
    let mut driver = PollDriver::new(shared);
    driver
      .upsert(watch(1, read_fd.as_raw_fd()), &OWNER_LIVE)
      .unwrap();

    write_byte(&write_fd);
    assert_eq!(wait_for_ready(&driver, 1).len(), 1);
    assert!(driver.drain_ready().is_empty());

    assert_eq!(driver.rearm(1, 1), Ok(()));
    assert_eq!(wait_for_ready(&driver, 1).len(), 1);
    driver.shutdown();
  }

  #[cfg(not(miri))] // needs I/O
  #[test]
  fn stop_discards_old_generation() {
    let (old_read, old_write) = pipe();
    let (new_read, new_write) = pipe();
    let (entered_second_poll, second_poll_entered) = mpsc::sync_channel(0);
    let barrier = Arc::new(Barrier::new(2));
    let poll_barrier = barrier.clone();
    let poll_calls = Arc::new(AtomicUsize::new(0));
    let poll_calls_for_worker = poll_calls.clone();
    let shared = LoopShared::new();
    let mut driver = PollDriver::new_with_poll_for_test(
      shared,
      Arc::new(move |fds, timeout| {
        if poll_calls_for_worker.fetch_add(1, Ordering::SeqCst) == 1 {
          let _ = entered_second_poll.send(());
          poll_barrier.wait();
        }
        default_poll(fds, timeout)
      }),
    );

    driver
      .upsert(watch(1, old_read.as_raw_fd()), &OWNER_LIVE)
      .unwrap();
    second_poll_entered
      .recv_timeout(DEADLINE)
      .expect("worker did not begin its second poll within {DEADLINE:?}");
    write_byte(&old_write);
    driver.stop(1, 1);
    driver
      .upsert(
        watch_with_generation(1, 2, new_read.as_raw_fd()),
        &OWNER_LIVE,
      )
      .unwrap();
    write_byte(&new_write);
    barrier.wait();

    let ready = wait_for_ready(&driver, 1);
    assert_eq!(
      ready,
      vec![PollReady {
        token: 1,
        generation: 2,
        status: 0,
        revents: libc::POLLIN
      }]
    );
    assert!(driver.drain_ready().is_empty());
    driver.shutdown();
  }

  #[cfg(not(miri))] // needs I/O
  #[test]
  fn permanent_poll_error_fails_all_watches_once() {
    let (read_a, _write_a) = pipe();
    let (read_b, _write_b) = pipe();
    let barrier = Arc::new(Barrier::new(2));
    let poll_barrier = barrier.clone();
    let shared = LoopShared::new();
    let mut driver = PollDriver::new_with_poll_for_test(
      shared,
      Arc::new(move |_, _| {
        poll_barrier.wait();
        Err(libc::EIO)
      }),
    );

    driver
      .upsert(watch(1, read_a.as_raw_fd()), &OWNER_LIVE)
      .unwrap();
    driver
      .upsert(watch(2, read_b.as_raw_fd()), &OWNER_LIVE)
      .unwrap();
    barrier.wait();

    let mut ready = wait_for_ready(&driver, 2);
    ready.sort_by_key(|ready| ready.token);
    assert_eq!(
      ready,
      vec![
        PollReady {
          token: 1,
          generation: 1,
          status: -libc::EIO,
          revents: 0
        },
        PollReady {
          token: 2,
          generation: 1,
          status: -libc::EIO,
          revents: 0
        },
      ]
    );
    assert_eq!(
      driver.upsert(watch(3, read_a.as_raw_fd()), &OWNER_LIVE),
      Err(libc::EIO)
    );
    assert!(driver.drain_ready().is_empty());
    driver.shutdown();
  }

  #[cfg(not(miri))] // needs I/O
  #[test]
  fn upsert_reports_failure_if_poll_fails_before_command_enqueue() {
    let (read_fd, _write_fd) = pipe();
    let (entered_sender, entered_receiver) = mpsc::sync_channel(0);
    let (release_sender, release_receiver) = mpsc::sync_channel(0);
    let failure_barrier = Arc::new(Barrier::new(2));
    let poll_barrier = failure_barrier.clone();
    let shared = LoopShared::new();
    let mut driver = PollDriver::new_with_poll_and_upsert_hook_for_test(
      shared,
      Arc::new(move |_, _| {
        poll_barrier.wait();
        Err(libc::EIO)
      }),
      PollUpsertHook {
        entered_sender,
        release_receiver,
      },
    );
    let control = driver.control.clone();
    let upsert = std::thread::spawn(move || {
      driver.upsert(watch(1, read_fd.as_raw_fd()), &OWNER_LIVE)
    });

    entered_receiver
      .recv_timeout(DEADLINE)
      .expect("upsert did not reach the command boundary within {DEADLINE:?}");
    failure_barrier.wait();
    let deadline = Instant::now() + DEADLINE;
    while lock_state(&control).permanent_failure != Some(libc::EIO) {
      assert!(
        Instant::now() < deadline,
        "poll failure was not recorded within {DEADLINE:?}"
      );
      std::thread::yield_now();
    }
    release_sender
      .send(())
      .expect("upsert did not wait for release");

    assert_eq!(
      upsert.join().expect("upsert thread panicked"),
      Err(libc::EIO)
    );
  }

  #[cfg(not(miri))] // needs I/O
  #[test]
  fn shutdown_joins_worker() {
    let (read_fd, _write_fd) = pipe();
    let shared = LoopShared::new();
    let mut driver = PollDriver::new(shared);
    driver
      .upsert(watch(1, read_fd.as_raw_fd()), &OWNER_LIVE)
      .unwrap();

    let (shutdown_complete, shutdown_done) = mpsc::sync_channel(1);
    let shutdown = std::thread::spawn(move || {
      driver.shutdown();
      let _ = shutdown_complete.send(());
    });
    shutdown_done
      .recv_timeout(DEADLINE)
      .expect("shutdown did not join promptly");
    assert!(shutdown.join().is_ok(), "shutdown thread panicked");
  }
}
