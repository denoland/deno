// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use signal_hook::consts::*;

#[cfg(windows)]
static SIGHUP: i32 = 1;

static COUNTER: AtomicU32 = AtomicU32::new(0);

type Handler = Box<dyn Fn() + Send>;
type Handlers = HashMap<i32, Vec<(u32, bool, Handler)>>;
static HANDLERS: OnceLock<(Handle, Mutex<Handlers>)> = OnceLock::new();

#[cfg(unix)]
struct Handle(signal_hook::iterator::Handle);

#[cfg(windows)]
struct Handle;

fn handle_signal(signal: i32) -> bool {
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

  let mut signals = Signals::new::<[i32; 0], i32>([]).unwrap();
  let handle = signals.handle();

  std::thread::spawn(move || {
    for signal in signals.forever() {
      let handled = handle_signal(signal);
      if !handled {
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

pub fn register(
  signal: i32,
  prevent_default: bool,
  f: Box<dyn Fn() + Send>,
) -> u32 {
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
        handle.0.add_signal(signal).unwrap();
      }
      #[cfg(windows)]
      {
        let _ = handle;
      }
    }
  }

  id
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
  register(SIGHUP, false, Box::new(f));
  register(SIGTERM, false, Box::new(f));
  register(SIGINT, false, Box::new(f));
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

pub fn is_forbidden(signo: i32) -> bool {
  FORBIDDEN.contains(&signo)
}
