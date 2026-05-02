// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use signal_hook::consts::*;
use tokio::sync::watch;

mod dict;
pub use dict::*;

#[cfg(windows)]
static SIGHUP: i32 = 1;
#[cfg(windows)]
static SIGWINCH: i32 = 28;

static COUNTER: AtomicU32 = AtomicU32::new(0);

type Handler = Box<dyn Fn() + Send>;
type Handlers = HashMap<i32, Vec<(u32, bool, Handler)>>;
static HANDLERS: OnceLock<(Handle, Mutex<Handlers>)> = OnceLock::new();

/// Interceptors run before regular handlers and can consume a signal.
/// If the interceptor returns `true`, the signal is consumed and regular
/// handlers are NOT called. Used by the SIGINT watchdog to prevent
/// other listeners (e.g. the test runner) from seeing the signal.
type Interceptor = Box<dyn Fn() -> bool + Send>;
static INTERCEPTORS: OnceLock<Mutex<HashMap<i32, Interceptor>>> =
  OnceLock::new();

#[cfg(unix)]
struct Handle(signal_hook::iterator::Handle);

#[cfg(windows)]
struct Handle;

fn handle_signal(signal: i32) -> bool {
  // Check interceptor first — if it consumes the signal, skip handlers.
  if let Some(interceptors) = INTERCEPTORS.get() {
    let interceptors = interceptors.lock().unwrap();
    if let Some(interceptor) = interceptors.get(&signal)
      && interceptor()
    {
      return true;
    }
  }

  let Some((_, handlers)) = HANDLERS.get() else {
    return false;
  };
  let handlers = handlers.lock().unwrap();
  let Some(handlers) = handlers.get(&signal) else {
    return false;
  };

  let mut handled = false;

  for (_, prevent_default, f) in handlers {
    if *prevent_default {
      handled = true;
    }

    f();
  }

  handled
}

#[cfg(unix)]
fn init() -> Handle {
  use signal_hook::iterator::Signals;

  let mut signals = Signals::new([SIGHUP, SIGTERM, SIGINT]).unwrap();
  let handle = signals.handle();

  std::thread::spawn(move || {
    for signal in signals.forever() {
      let handled = handle_signal(signal);
      if !handled {
        if signal == SIGHUP || signal == SIGTERM || signal == SIGINT {
          run_exit();
        }
        signal_hook::low_level::emulate_default_handler(signal).unwrap();
      }
    }
  });

  Handle(handle)
}

#[cfg(windows)]
fn init() -> Handle {
  unsafe extern "system" fn handle(ctrl_type: u32) -> i32 {
    let signal = match ctrl_type {
      0 => SIGINT,
      1 => SIGBREAK,
      2 => SIGHUP,
      5 => SIGTERM,
      6 => SIGTERM,
      _ => return 0,
    };
    let handled = handle_signal(signal);
    handled as _
  }

  // SAFETY: Registering handler
  unsafe {
    winapi::um::consoleapi::SetConsoleCtrlHandler(Some(handle), 1);
  }

  Handle
}

#[cfg(windows)]
fn start_sigwinch_polling() {
  static STARTED: OnceLock<()> = OnceLock::new();
  STARTED.get_or_init(|| {
    std::thread::spawn(|| {
      // SAFETY: winapi calls to open CONOUT$ and poll console size
      unsafe {
        let conout_name: Vec<u16> =
          "CONOUT$".encode_utf16().chain(Some(0)).collect();
        let handle = winapi::um::fileapi::CreateFileW(
          conout_name.as_ptr(),
          winapi::um::winnt::GENERIC_READ,
          winapi::um::winnt::FILE_SHARE_READ
            | winapi::um::winnt::FILE_SHARE_WRITE,
          std::ptr::null_mut(),
          winapi::um::fileapi::OPEN_EXISTING,
          0,
          std::ptr::null_mut(),
        );
        if handle == winapi::um::handleapi::INVALID_HANDLE_VALUE {
          return;
        }

        let mut prev_cols: i32 = 0;
        let mut prev_rows: i32 = 0;

        // Read initial size
        let mut bufinfo: winapi::um::wincon::CONSOLE_SCREEN_BUFFER_INFO =
          std::mem::zeroed();
        if winapi::um::wincon::GetConsoleScreenBufferInfo(handle, &mut bufinfo)
          != 0
        {
          prev_cols =
            bufinfo.srWindow.Right as i32 - bufinfo.srWindow.Left as i32 + 1;
          prev_rows =
            bufinfo.srWindow.Bottom as i32 - bufinfo.srWindow.Top as i32 + 1;
        }

        loop {
          winapi::um::synchapi::Sleep(250);

          let mut bufinfo: winapi::um::wincon::CONSOLE_SCREEN_BUFFER_INFO =
            std::mem::zeroed();
          if winapi::um::wincon::GetConsoleScreenBufferInfo(
            handle,
            &mut bufinfo,
          ) == 0
          {
            continue;
          }

          let cols =
            bufinfo.srWindow.Right as i32 - bufinfo.srWindow.Left as i32 + 1;
          let rows =
            bufinfo.srWindow.Bottom as i32 - bufinfo.srWindow.Top as i32 + 1;

          if cols != prev_cols || rows != prev_rows {
            prev_cols = cols;
            prev_rows = rows;
            handle_signal(SIGWINCH);
          }
        }
      }
    });
  });
}

pub fn register(
  signal: i32,
  prevent_default: bool,
  f: Box<dyn Fn() + Send>,
) -> Result<u32, std::io::Error> {
  if is_forbidden(signal) {
    return Err(std::io::Error::other(format!(
      "Refusing to register signal {signal}"
    )));
  }

  let (handle, handlers) = HANDLERS.get_or_init(|| {
    let handle = init();

    let handlers = Mutex::new(HashMap::new());

    (handle, handlers)
  });

  let id = COUNTER.fetch_add(1, Ordering::Relaxed);
  let mut handlers = handlers.lock().unwrap();
  match handlers.entry(signal) {
    std::collections::hash_map::Entry::Occupied(mut v) => {
      v.get_mut().push((id, prevent_default, f))
    }
    std::collections::hash_map::Entry::Vacant(v) => {
      v.insert(vec![(id, prevent_default, f)]);

      #[cfg(unix)]
      {
        handle.0.add_signal(signal).map_err(|e| {
          std::io::Error::other(format!(
            "Failed to register signal {signal}: {e}"
          ))
        })?;
      }
      #[cfg(windows)]
      {
        let _ = handle;
        if signal == SIGWINCH {
          start_sigwinch_polling();
        }
      }
    }
  }

  Ok(id)
}

/// Set an interceptor for a signal. The interceptor runs before regular
/// handlers. If it returns `true`, the signal is consumed and no regular
/// handlers are called. Pass `None` to remove the interceptor.
pub fn set_interceptor(signal: i32, f: Option<Box<dyn Fn() -> bool + Send>>) {
  // Ensure signal infrastructure is initialized so the signal thread
  // exists to call handle_signal (and thus our interceptor).
  HANDLERS.get_or_init(|| {
    let handle = init();
    let handlers = Mutex::new(HashMap::new());
    (handle, handlers)
  });

  let interceptors = INTERCEPTORS.get_or_init(|| Mutex::new(HashMap::new()));
  let mut map = interceptors.lock().unwrap();
  match f {
    Some(f) => {
      map.insert(signal, f);
    }
    None => {
      map.remove(&signal);
    }
  }
}

pub fn unregister(signal: i32, id: u32) {
  let Some((_, handlers)) = HANDLERS.get() else {
    return;
  };
  let mut handlers = handlers.lock().unwrap();
  let Some(handlers) = handlers.get_mut(&signal) else {
    return;
  };
  let Some(index) = handlers.iter().position(|v| v.0 == id) else {
    return;
  };
  let _ = handlers.swap_remove(index);
}

static BEFORE_EXIT: OnceLock<Mutex<Vec<Handler>>> = OnceLock::new();

pub fn before_exit(f: fn()) {
  BEFORE_EXIT
    .get_or_init(|| Mutex::new(vec![]))
    .lock()
    .unwrap()
    .push(Box::new(f));
}

pub fn run_exit() {
  if let Some(fns) = BEFORE_EXIT.get() {
    let fns = fns.lock().unwrap();
    for f in fns.iter() {
      f();
    }
  }
}

pub const SIGINT: i32 = 2;
pub const SIGTERM: i32 = 15;

/// Synthetically raise a signal, triggering all registered JS handlers.
///
/// This does NOT use OS-level signal delivery — it directly invokes the
/// handler functions under a mutex, making it safe to call from any async
/// or sync context on all platforms (including Windows).
///
/// Returns true if any handler prevented the default behavior.
pub fn raise(signal: i32) -> bool {
  handle_signal(signal)
}

pub fn is_forbidden(signo: i32) -> bool {
  if FORBIDDEN.contains(&signo) {
    return true;
  }
  // On Windows, signal_hook's FORBIDDEN list doesn't include SIGKILL/SIGABRT
  // (they're Unix-specific in the crate). Add them here since listening for
  // uncatchable/fatal signals doesn't make sense on any platform.
  #[cfg(windows)]
  if signo == 9 || signo == 22 {
    // SIGKILL (9) and SIGABRT (22, Windows CRT value)
    return true;
  }
  false
}

pub struct SignalStream {
  rx: watch::Receiver<()>,
}

impl SignalStream {
  pub async fn recv(&mut self) -> Option<()> {
    self.rx.changed().await.ok()
  }
}

pub fn signal_stream(signo: i32) -> Result<SignalStream, std::io::Error> {
  let (tx, rx) = watch::channel(());
  let cb = Box::new(move || {
    tx.send_replace(());
  });
  register(signo, true, cb)?;
  Ok(SignalStream { rx })
}

pub async fn ctrl_c() -> std::io::Result<()> {
  let mut stream = signal_stream(libc::SIGINT)?;
  match stream.recv().await {
    Some(_) => Ok(()),
    None => Err(std::io::Error::other("failed to receive SIGINT signal")),
  }
}
