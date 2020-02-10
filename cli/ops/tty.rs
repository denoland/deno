use super::dispatch_json::JsonOp;
use super::io::StreamResource;
use crate::deno_error::bad_resource;
use crate::deno_error::other_error;
use crate::ops::json_op;
use crate::state::State;
use deno_core::*;
#[cfg(unix)]
use nix::sys::termios;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("set_raw", s.core_op(json_op(s.stateful_op(op_set_raw))));
}

#[cfg(windows)]
macro_rules! wincheck {
  ($funcall:expr) => {{
    let rc = unsafe { $funcall };
    if rc == 0 {
      Err(ErrBox::from(std::io::Error::last_os_error()))?;
    }
    rc
  }};
}

// libc::termios cannot be serialized.
// Create a similar one for our use.
#[cfg(unix)]
#[derive(Serialize, Deserialize)]
struct SerializedTermios {
  iflags: libc::tcflag_t,
  oflags: libc::tcflag_t,
  cflags: libc::tcflag_t,
  lflags: libc::tcflag_t,
  cc: [libc::cc_t; libc::NCCS],
}

#[cfg(unix)]
impl From<termios::Termios> for SerializedTermios {
  fn from(t: termios::Termios) -> Self {
    Self {
      iflags: t.input_flags.bits(),
      oflags: t.output_flags.bits(),
      cflags: t.control_flags.bits(),
      lflags: t.local_flags.bits(),
      cc: t.control_chars,
    }
  }
}

#[cfg(unix)]
impl Into<termios::Termios> for SerializedTermios {
  fn into(self) -> termios::Termios {
    let mut t = unsafe { termios::Termios::default_uninit() };
    t.input_flags = termios::InputFlags::from_bits_truncate(self.iflags);
    t.output_flags = termios::OutputFlags::from_bits_truncate(self.oflags);
    t.control_flags = termios::ControlFlags::from_bits_truncate(self.cflags);
    t.local_flags = termios::LocalFlags::from_bits_truncate(self.lflags);
    t.control_chars = self.cc;
    t
  }
}

#[derive(Deserialize)]
struct SetRawArgs {
  rid: u32,
  raw: bool,
  #[cfg(unix)]
  restore: Option<String>, // Only used for *nix
                           // Saved as string in case of u64 problem in JS
}

pub fn op_set_raw(
  state_: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SetRawArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let is_raw = args.raw;

  let state = state_.borrow_mut();
  let resource = state.resource_table.get::<StreamResource>(rid);
  if resource.is_none() {
    return Err(bad_resource());
  }

  // For now, only stdin
  match resource.unwrap() {
    StreamResource::Stdin(_) => (),
    _ => {
      return Err(other_error(
        "Resource other than stdin is not implemented".to_owned(),
      ))
    }
  }

  // From https://github.com/kkawakam/rustyline/blob/master/src/tty/windows.rs
  // and https://github.com/crossterm-rs/crossterm/blob/e35d4d2c1cc4c919e36d242e014af75f6127ab50/src/terminal/sys/windows.rs
  #[cfg(windows)]
  {
    use std::os::windows::io::AsRawHandle;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::{consoleapi, handleapi, wincon};

    let handle = std::io::stdin().as_raw_handle();
    if handle == handleapi::INVALID_HANDLE_VALUE {
      return Err(ErrBox::from(std::io::Error::last_os_error()));
    } else if handle.is_null() {
      return Err(ErrBox::from(other_error("null handle".to_owned())));
    }
    let mut original_mode: DWORD = 0;
    let RAW_MODE_MASK = wincon::ENABLE_LINE_INPUT
      | wincon::ENABLE_ECHO_INPUT
      | wincon::ENABLE_PROCESSED_INPUT;
    wincheck!(consoleapi::GetConsoleMode(handle, &mut original_mode));
    let new_mode = if is_raw {
      original_mode & !RAW_MODE_MASK;
    } else {
      original_mode | RAW_MODE_MASK;
    };
    wincheck!(consoleapi::SetConsoleMode(handle, new_mode));

    Ok(JsonOp::Sync(json!({})))
  }
  // From https://github.com/kkawakam/rustyline/blob/master/src/tty/unix.rs
  #[cfg(unix)]
  {
    use std::os::unix::io::AsRawFd;
    let raw_fd = std::io::stdin().as_raw_fd();

    if is_raw {
      let original_mode = termios::tcgetattr(raw_fd)?;
      let mut raw = original_mode.clone();

      raw.input_flags &= !(termios::InputFlags::BRKINT
        | termios::InputFlags::ICRNL
        | termios::InputFlags::INPCK
        | termios::InputFlags::ISTRIP
        | termios::InputFlags::IXON);

      raw.control_flags |= termios::ControlFlags::CS8;

      raw.local_flags &= !(termios::LocalFlags::ECHO
        | termios::LocalFlags::ICANON
        | termios::LocalFlags::IEXTEN
        | termios::LocalFlags::ISIG);
      raw.control_chars[termios::SpecialCharacterIndices::VMIN as usize] = 1;
      raw.control_chars[termios::SpecialCharacterIndices::VTIME as usize] = 0;
      termios::tcsetattr(raw_fd, termios::SetArg::TCSADRAIN, &raw)?;
      Ok(JsonOp::Sync(json!({
        "restore":
          serde_json::to_string(&SerializedTermios::from(original_mode)).unwrap(),
      })))
    } else {
      // Restore old mode.
      if args.restore.is_none() {
        return Err(other_error("no termios to restore".to_owned()));
      }
      let old_termios =
        serde_json::from_str::<SerializedTermios>(&args.restore.unwrap());
      if old_termios.is_err() {
        return Err(other_error("bad termios to restore".to_owned()));
      }
      let old_termios = old_termios.unwrap();
      termios::tcsetattr(
        raw_fd,
        termios::SetArg::TCSADRAIN,
        &old_termios.into(),
      )?;
      Ok(JsonOp::Sync(json!({})))
    }
  }
}
