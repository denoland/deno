#![allow(non_snake_case)]

use deno_core::{
  CppgcInherits, GarbageCollected, OpState,
  cppgc::try_unwrap_cppgc_object,
  op2,
  uv_compat::{
    UV_EBADF, uv_guess_handle, uv_handle_type, uv_loop_t, uv_tty_get_winsize,
    uv_tty_init, uv_tty_mode_t, uv_tty_set_mode, uv_tty_t,
  },
  v8,
};

use crate::ops::handle_wrap::AsyncWrap;
use crate::ops::handle_wrap::Handle;
use crate::ops::handle_wrap::HandleWrap;
use crate::ops::handle_wrap::ProviderType;
use crate::ops::stream_wrap::LibUvStreamWrap;

pub struct OwnedPtr<T>(*mut T);

impl<T> OwnedPtr<T> {
  pub fn from_box(b: Box<T>) -> Self {
    Self(Box::into_raw(b))
  }

  pub fn as_mut_ptr(&self) -> *mut T {
    self.0
  }

  pub fn as_ptr(&self) -> *const T {
    self.0
  }

  pub unsafe fn cast<U>(self) -> OwnedPtr<U> {
    const {
      assert!(size_of::<T>() == size_of::<U>());
      assert!(align_of::<T>() == align_of::<U>());
    }

    OwnedPtr(self.0.cast())
  }
}

impl<T> Drop for OwnedPtr<T> {
  fn drop(&mut self) {
    unsafe {
      let _ = Box::from_raw(self.0);
    }
  }
}

#[derive(CppgcInherits)]
#[cppgc_inherits_from(LibUvStreamWrap)]
#[repr(C)]
pub struct TTY {
  base: LibUvStreamWrap,
  pub(crate) handle: OwnedPtr<uv_tty_t>,
}

unsafe impl GarbageCollected for TTY {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TTY"
  }

  fn trace(&self, visitor: &mut deno_core::v8::cppgc::Visitor) {
    self.base.trace(visitor);
  }
}

impl TTY {
  pub fn new(
    _obj: v8::Local<v8::Object>,
    fd: i32,
    op_state: &mut deno_core::OpState,
  ) -> Self {
    // todo: uv_stream_t thing + LibuvStreamWrap init

    // todo: this should really not be a Box<uv_loop_t> because of uniqueness guarantees.
    // instead it should be a custom wrapper around a raw pointer
    // right now this is most likely ub
    let loop_ = &**op_state.borrow::<Box<uv_loop_t>>() as *const uv_loop_t
      as *mut uv_loop_t;

    let tty = OwnedPtr::from_box(Box::<uv_tty_t>::new_uninit());

    let err = unsafe { uv_tty_init(loop_, tty.as_mut_ptr().cast(), fd, 0) };

    if err == 0 {
      let tty = unsafe { tty.cast::<uv_tty_t>() };
      Self {
        base: LibUvStreamWrap::new(
          HandleWrap::create(
            AsyncWrap::create(op_state, ProviderType::TtyWrap as i32),
            Some(Handle::New(tty.as_ptr().cast())),
          ),
          fd,
          tty.as_ptr().cast(),
        ),
        handle: tty,
      }
    } else {
      panic!("Failed to initialize TTY: {}", err);
    }
  }
}

#[op2(inherit = LibUvStreamWrap)]
impl TTY {
  #[constructor]
  #[cppgc]
  pub fn new_tty(
    fd: i32,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope,
    op_state: &mut OpState,
  ) -> TTY {
    assert!(fd >= 0);

    let obj = v8::Local::new(scope, &this);
    TTY::new(obj, fd, op_state)
  }

  #[fast]
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
    let handle = self.handle.as_mut_ptr();

    let (mut width, mut height) = (0, 0);
    let err = unsafe { uv_tty_get_winsize(handle, &mut width, &mut height) };

    if err == 0 {
      if a
        .set_index(scope, 0, v8::Integer::new(scope, width).into())
        .is_none()
        || a
          .set_index(scope, 1, v8::Integer::new(scope, height).into())
          .is_none()
      {
        return -1;
      }
    }

    err
  }

  #[fast]
  pub fn set_raw_mode(&self, arg: v8::Local<v8::Value>) -> i32 {
    let err = unsafe {
      uv_tty_set_mode(
        self.handle.as_mut_ptr(),
        if arg.is_true() {
          uv_tty_mode_t::UV_TTY_MODE_RAW_VT
        } else {
          uv_tty_mode_t::UV_TTY_MODE_NORMAL
        },
      )
    };

    err
  }
}
