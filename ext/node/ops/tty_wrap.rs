// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::CppgcInherits;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;
use deno_core::uv_compat::UV_EADDRINUSE;
use deno_core::uv_compat::UV_EAGAIN;
use deno_core::uv_compat::UV_EBADF;
use deno_core::uv_compat::UV_EBUSY;
use deno_core::uv_compat::UV_ECANCELED;
use deno_core::uv_compat::UV_ECONNREFUSED;
use deno_core::uv_compat::UV_EINVAL;
use deno_core::uv_compat::UV_ENOBUFS;
use deno_core::uv_compat::UV_ENOTCONN;
use deno_core::uv_compat::UV_ENOTSUP;
use deno_core::uv_compat::UV_EPIPE;
use deno_core::uv_compat::uv_guess_handle;
use deno_core::uv_compat::uv_handle_type;
use deno_core::uv_compat::uv_loop_t;
use deno_core::uv_compat::uv_tty_get_winsize;
use deno_core::uv_compat::uv_tty_init;
use deno_core::uv_compat::uv_tty_mode_t;
use deno_core::uv_compat::uv_tty_set_mode;
use deno_core::uv_compat::uv_tty_t;
use deno_core::v8;

/// Map a uv error code to (name, message) matching libuv's uv_err_name/uv_strerror.
fn uv_error_info(err: i32) -> (&'static str, &'static str) {
  match err {
    x if x == UV_EAGAIN => ("EAGAIN", "resource temporarily unavailable"),
    x if x == UV_EADDRINUSE => ("EADDRINUSE", "address already in use"),
    x if x == UV_EBADF => ("EBADF", "bad file descriptor"),
    x if x == UV_EBUSY => ("EBUSY", "resource busy or locked"),
    x if x == UV_ECANCELED => ("ECANCELED", "operation canceled"),
    x if x == UV_ECONNREFUSED => ("ECONNREFUSED", "connection refused"),
    x if x == UV_EINVAL => ("EINVAL", "invalid argument"),
    x if x == UV_ENOBUFS => ("ENOBUFS", "no buffer space available"),
    x if x == UV_ENOTCONN => ("ENOTCONN", "socket is not connected"),
    x if x == UV_ENOTSUP => ("ENOTSUP", "operation not supported on socket"),
    x if x == UV_EPIPE => ("EPIPE", "broken pipe"),
    _ => ("UNKNOWN", "unknown error"),
  }
}

use deno_permissions::PermissionsContainer;

use crate::ops::handle_wrap::AsyncWrap;
use crate::ops::handle_wrap::Handle;
use crate::ops::handle_wrap::HandleWrap;
use crate::ops::handle_wrap::OwnedPtr;
use crate::ops::handle_wrap::ProviderType;
use crate::ops::stream_wrap::LibUvStreamWrap;

/// Check that non-stdio file descriptors (fd > 2) have --allow-all permission.
/// Stdio fds 0, 1, 2 are always allowed.
#[op2(fast)]
pub fn op_tty_check_fd_permission(
  state: &mut OpState,
  fd: i32,
) -> Result<(), deno_permissions::PermissionCheckError> {
  if fd > 2 {
    state
      .borrow_mut::<PermissionsContainer>()
      .check_read_all("node:tty TTY()")?;
    state
      .borrow_mut::<PermissionsContainer>()
      .check_write_all("node:tty TTY()")?;
  }
  Ok(())
}

#[derive(CppgcInherits)]
#[cppgc_inherits_from(LibUvStreamWrap)]
#[repr(C)]
pub struct TTY {
  base: LibUvStreamWrap,
  pub(crate) handle: Option<OwnedPtr<uv_tty_t>>,
}

// SAFETY: TTY is a cppgc-managed object; the GC correctly traces it via the base field.
unsafe impl GarbageCollected for TTY {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TTY"
  }

  fn trace(&self, visitor: &mut deno_core::v8::cppgc::Visitor) {
    self.base.trace(visitor);
  }
}

impl Drop for TTY {
  fn drop(&mut self) {
    self.base.detach_stream();
  }
}

impl TTY {
  pub fn new(
    _obj: v8::Local<v8::Object>,
    fd: i32,
    op_state: &mut deno_core::OpState,
  ) -> (Self, i32) {
    // todo: this should really not be a Box<uv_loop_t> because of uniqueness guarantees.
    // instead it should be a custom wrapper around a raw pointer
    // right now this is most likely ub
    let loop_ = &**op_state.borrow::<Box<uv_loop_t>>() as *const uv_loop_t
      as *mut uv_loop_t;

    let tty = OwnedPtr::from_box(Box::<uv_tty_t>::new_uninit());

    // SAFETY: loop_ is a valid uv_loop_t pointer from OpState; tty points to uninit memory of the right size for uv_tty_t; fd is a valid file descriptor.
    let err = unsafe { uv_tty_init(loop_, tty.as_mut_ptr().cast(), fd, 0) };

    if err == 0 {
      // SAFETY: uv_tty_init succeeded so the memory is fully initialized as a uv_tty_t.
      let tty = unsafe { tty.cast::<uv_tty_t>() };
      let base = LibUvStreamWrap::new(
        HandleWrap::create(
          AsyncWrap::create(op_state, ProviderType::TtyWrap as i32),
          Some(Handle::New(tty.as_ptr().cast())),
        ),
        fd,
        tty.as_ptr().cast(),
      );
      // SAFETY: tty pointer is valid and initialized; setting data field for libuv callbacks.
      unsafe {
        (*tty.as_mut_ptr()).data = base.handle_data_ptr();
      }
      (
        Self {
          base,
          handle: Some(tty),
        },
        0,
      )
    } else {
      // Match Node: don't panic, return uninitialized handle with error code.
      // Free the uninit allocation without dropping as uv_tty_t.
      // SAFETY: tty.0 was allocated via Box::new_uninit with the layout of uv_tty_t; we free it directly because the memory was never initialized and must not be dropped as uv_tty_t.
      unsafe {
        let layout = std::alloc::Layout::new::<uv_tty_t>();
        std::alloc::dealloc(tty.as_mut_ptr() as *mut u8, layout);
        std::mem::forget(tty);
      }
      (
        Self {
          base: LibUvStreamWrap::new(
            HandleWrap::create(
              AsyncWrap::create(op_state, ProviderType::TtyWrap as i32),
              None,
            ),
            fd,
            std::ptr::null(),
          ),
          handle: None,
        },
        err,
      )
    }
  }
}

#[op2(inherit = LibUvStreamWrap)]
impl TTY {
  #[constructor]
  #[cppgc]
  pub fn new_tty(
    fd: i32,
    ctx: v8::Local<v8::Value>,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> TTY {
    assert!(fd >= 0);

    let obj = v8::Local::new(scope, &this);
    let (tty, err) = TTY::new(obj, fd, op_state);
    if err != 0
      && let Ok(ctx_obj) = v8::Local::<v8::Object>::try_from(ctx)
    {
      let (code_name, message) = uv_error_info(err);

      let code_key =
        v8::String::new_external_onebyte_static(scope, b"code").unwrap();
      let code_str = v8::String::new(scope, code_name).unwrap();
      ctx_obj.set(scope, code_key.into(), code_str.into());

      let msg_key =
        v8::String::new_external_onebyte_static(scope, b"message").unwrap();
      let msg_str = v8::String::new(scope, message).unwrap();
      ctx_obj.set(scope, msg_key.into(), msg_str.into());

      let errno_key =
        v8::String::new_external_onebyte_static(scope, b"errno").unwrap();
      let errno_val = v8::Integer::new(scope, err);
      ctx_obj.set(scope, errno_key.into(), errno_val.into());

      let syscall_key =
        v8::String::new_external_onebyte_static(scope, b"syscall").unwrap();
      let syscall_str =
        v8::String::new_external_onebyte_static(scope, b"uv_tty_init").unwrap();
      ctx_obj.set(scope, syscall_key.into(), syscall_str.into());
    }
    tty
  }

  #[fast]
  #[rename("isTTY")]
  #[static_method]
  pub fn is_TTY(fd: i32) -> bool {
    assert!(fd >= 0);

    uv_guess_handle(fd) == uv_handle_type::UV_TTY
  }

  #[fast]
  #[no_side_effects]
  pub fn get_window_size(
    &self,
    a: v8::Local<v8::Array>,
    scope: &mut v8::PinScope,
  ) -> i32 {
    let Some(ref handle) = self.handle else {
      return UV_EBADF;
    };
    let handle = handle.as_mut_ptr();

    let (mut width, mut height) = (0, 0);
    // SAFETY: handle is a valid initialized uv_tty_t pointer, verified above via the Some guard.
    let err = unsafe { uv_tty_get_winsize(handle, &mut width, &mut height) };

    if err == 0
      && (a
        .set_index(scope, 0, v8::Integer::new(scope, width).into())
        .is_none()
        || a
          .set_index(scope, 1, v8::Integer::new(scope, height).into())
          .is_none())
    {
      return -1;
    }

    err
  }

  #[fast]
  pub fn set_raw_mode(&self, arg: v8::Local<v8::Value>) -> i32 {
    let Some(ref handle) = self.handle else {
      return UV_EBADF;
    };
    // SAFETY: handle is a valid initialized uv_tty_t pointer, verified above via the Some guard.
    unsafe {
      uv_tty_set_mode(
        handle.as_mut_ptr(),
        if arg.is_true() {
          uv_tty_mode_t::UV_TTY_MODE_RAW_VT
        } else {
          uv_tty_mode_t::UV_TTY_MODE_NORMAL
        },
      )
    }
  }
}
