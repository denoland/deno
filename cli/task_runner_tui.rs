// Copyright 2018-2026 the Deno authors. MIT license.

//! Experimental interactive TUI for `deno task` parallel runs.
//!
//! Opt-in via the `DENO_TASK_TUI=1` environment variable. When enabled and
//! multiple tasks run concurrently in an interactive terminal, each task's
//! output is captured into its own scrollback buffer and rendered in a
//! turborepo-style view: a sidebar list of tasks plus a focused output pane.
//! Use Up/Down (or k/j) to switch the focused task and `q` (or Ctrl+C) to quit.
//!
//! This is a prototype: it is read-only (no input forwarding to tasks) and does
//! not allocate a PTY per task, so child programs that probe for a TTY may
//! render differently than they would when attached directly.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::Arc;
use std::thread::JoinHandle as ThreadJoinHandle;
use std::time::Duration;

use console_static_text::TextItem;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use deno_core::parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::colors;
use crate::task_runner::TaskIo;
use crate::task_runner::make_task_io_with_writers;
use crate::util::console::new_console_static_text;

/// Maximum number of output lines retained per task.
const MAX_LINES_PER_TASK: usize = 2000;

struct TuiTask {
  label: String,
  lines: VecDeque<String>,
  finished: bool,
}

struct TuiState {
  tasks: Vec<TuiTask>,
  focused: usize,
  /// Set when the render loop should stop (all tasks done, or user quit).
  done: bool,
}

impl TuiState {
  fn push_line(&mut self, idx: usize, line: String) {
    if let Some(task) = self.tasks.get_mut(idx) {
      if task.lines.len() >= MAX_LINES_PER_TASK {
        task.lines.pop_front();
      }
      task.lines.push_back(line);
    }
  }
}

/// Cheaply-cloneable handle to the shared TUI state.
#[derive(Clone)]
pub struct TaskUi {
  state: Arc<Mutex<TuiState>>,
}

impl TaskUi {
  pub fn new() -> Self {
    Self {
      state: Arc::new(Mutex::new(TuiState {
        tasks: Vec::new(),
        focused: 0,
        done: false,
      })),
    }
  }

  /// Registers a task by display label, returning its index.
  fn register(&self, label: String) -> usize {
    let mut state = self.state.lock();
    state.tasks.push(TuiTask {
      label,
      lines: VecDeque::new(),
      finished: false,
    });
    state.tasks.len() - 1
  }

  /// Creates a [`TaskIo`] that routes the task's output into its own buffer.
  /// Returns the io, the task's TUI index, and the reader join handles.
  pub fn make_task_io(
    &self,
    label: String,
  ) -> (TaskIo, usize, Vec<JoinHandle<()>>) {
    let idx = self.register(label);
    let out = Box::new(BufferWriter::new(self.state.clone(), idx));
    let err = Box::new(BufferWriter::new(self.state.clone(), idx));
    let (io, handles) = make_task_io_with_writers(out, err);
    (io, idx, handles)
  }

  pub fn mark_finished(&self, idx: usize) {
    if let Some(task) = self.state.lock().tasks.get_mut(idx) {
      task.finished = true;
    }
  }

  /// Signals the render loop to stop (all tasks complete).
  pub fn finish(&self) {
    self.state.lock().done = true;
  }
}

/// A line-buffering [`Write`] that appends completed lines to a task's buffer.
struct BufferWriter {
  state: Arc<Mutex<TuiState>>,
  idx: usize,
  line_buf: Vec<u8>,
}

impl BufferWriter {
  fn new(state: Arc<Mutex<TuiState>>, idx: usize) -> Self {
    Self {
      state,
      idx,
      line_buf: Vec::new(),
    }
  }

  fn flush_line(&mut self) {
    let line = String::from_utf8_lossy(&self.line_buf).into_owned();
    self.state.lock().push_line(self.idx, line);
    self.line_buf.clear();
  }
}

impl Write for BufferWriter {
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    for &b in buf {
      match b {
        b'\n' => self.flush_line(),
        b'\r' => {}
        _ => self.line_buf.push(b),
      }
    }
    Ok(buf.len())
  }

  fn flush(&mut self) -> std::io::Result<()> {
    if !self.line_buf.is_empty() {
      self.flush_line();
    }
    Ok(())
  }
}

/// Returns true when the task TUI is opted into and the terminal is interactive.
pub fn tui_enabled() -> bool {
  use std::io::IsTerminal;
  std::env::var("DENO_TASK_TUI").is_ok_and(|v| v != "0" && !v.is_empty())
    && std::io::stderr().is_terminal()
}

/// Spawns the render loop on a dedicated OS thread. Returns its handle so the
/// caller can join after calling [`TaskUi::finish`].
///
/// `quit_tx` is signalled when the user requests to quit; the async run loop
/// owns the (non-`Send`) [`KillSignal`] and performs the actual termination.
pub fn spawn_render_thread(
  ui: TaskUi,
  quit_tx: UnboundedSender<()>,
) -> ThreadJoinHandle<()> {
  std::thread::spawn(move || {
    let _ = crossterm::terminal::enable_raw_mode();
    let mut static_text = new_console_static_text();
    static_text.keep_cursor_zero_column(true);

    loop {
      let done = {
        let state = ui.state.lock();
        static_text.eprint_items(render(&state, &static_text).iter());
        state.done
      };
      if done {
        break;
      }

      if crossterm::event::poll(Duration::from_millis(120)).unwrap_or(false) {
        if let Ok(Event::Key(KeyEvent {
          code,
          modifiers,
          kind,
          ..
        })) = crossterm::event::read()
        {
          if kind != KeyEventKind::Release {
            handle_key(&ui, &quit_tx, code, modifiers);
          }
        }
      }
    }

    static_text.eprint_clear();
    let _ = crossterm::terminal::disable_raw_mode();
  })
}

fn handle_key(
  ui: &TaskUi,
  quit_tx: &UnboundedSender<()>,
  code: KeyCode,
  modifiers: KeyModifiers,
) {
  let mut state = ui.state.lock();
  let count = state.tasks.len();
  match (code, modifiers) {
    (KeyCode::Up | KeyCode::Char('k'), _) => {
      state.focused = state.focused.saturating_sub(1);
    }
    (KeyCode::Down | KeyCode::Char('j'), _) => {
      if state.focused + 1 < count {
        state.focused += 1;
      }
    }
    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
      state.done = true;
      drop(state);
      let _ = quit_tx.send(());
    }
    _ => {}
  }
}

fn render(
  state: &TuiState,
  static_text: &console_static_text::ConsoleStaticText,
) -> Vec<TextItem<'static>> {
  let mut items = Vec::new();
  items.push(TextItem::new_owned(
    colors::gray("deno task  (\u{2191}/\u{2193} switch \u{2022} q quit)")
      .to_string(),
  ));

  for (i, task) in state.tasks.iter().enumerate() {
    let focused = i == state.focused;
    let marker = if focused { "\u{25b6}" } else { " " };
    let status = if task.finished {
      colors::green("done").to_string()
    } else {
      colors::yellow("running").to_string()
    };
    let label = if focused {
      colors::intense_blue(&task.label).to_string()
    } else {
      task.label.clone()
    };
    items.push(TextItem::new_owned(format!("{marker} {label} [{status}]")));
  }

  items.push(TextItem::new_owned(
    colors::gray("\u{2500}".repeat(40)).to_string(),
  ));

  if let Some(task) = state.tasks.get(state.focused) {
    let header_rows = state.tasks.len() + 2;
    let avail = static_text
      .console_size()
      .rows
      .map(|rows| (rows as usize).saturating_sub(header_rows + 1))
      .unwrap_or(20)
      .max(1);
    let start = task.lines.len().saturating_sub(avail);
    for line in task.lines.iter().skip(start) {
      items.push(TextItem::new_owned(line.clone()));
    }
  }

  items
}
