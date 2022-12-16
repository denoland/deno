// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::include_js_files;
use deno_core::v8;
use deno_core::Extension;
use deno_core::OpState;

use std::cell::RefCell;
use std::mem::size_of;
use std::os::raw::c_char;
use std::os::raw::c_short;
use std::path::Path;
use std::ptr;
use std::rc::Rc;

mod call;
mod callback;
mod dlfcn;
mod ir;
mod repr;
mod r#static;
mod symbol;
mod turbocall;

use call::op_ffi_call_nonblocking;
use call::op_ffi_call_ptr;
use call::op_ffi_call_ptr_nonblocking;
use callback::op_ffi_unsafe_callback_create;
use callback::op_ffi_unsafe_callback_ref;
use callback::op_ffi_unsafe_callback_unref;
use dlfcn::op_ffi_load;
use dlfcn::ForeignFunction;
use r#static::op_ffi_get_static;
use repr::*;
use symbol::NativeType;
use symbol::Symbol;

#[cfg(not(target_pointer_width = "64"))]
compile_error!("platform not supported");

const _: () = {
  assert!(size_of::<c_char>() == 1);
  assert!(size_of::<c_short>() == 2);
  assert!(size_of::<*const ()>() == 8);
};

thread_local! {
  static LOCAL_ISOLATE_POINTER: RefCell<*const v8::Isolate> = RefCell::new(ptr::null());
}

pub(crate) const MAX_SAFE_INTEGER: isize = 9007199254740991;
pub(crate) const MIN_SAFE_INTEGER: isize = -9007199254740991;

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

pub fn check_unstable2(state: &Rc<RefCell<OpState>>, api_name: &str) {
  let state = state.borrow();
  check_unstable(&state, api_name)
}

pub trait FfiPermissions {
  fn check(&mut self, path: Option<&Path>) -> Result<(), AnyError>;
}

pub(crate) type PendingFfiAsyncWork = Box<dyn FnOnce()>;

pub(crate) struct FfiState {
  pub(crate) async_work_sender: mpsc::UnboundedSender<PendingFfiAsyncWork>,
  pub(crate) async_work_receiver: mpsc::UnboundedReceiver<PendingFfiAsyncWork>,
}

pub fn init<P: FfiPermissions + 'static>(unstable: bool) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/ffi",
      "00_ffi.js",
    ))
    .ops(vec![
      op_ffi_load::decl::<P>(),
      op_ffi_get_static::decl(),
      op_ffi_call_nonblocking::decl(),
      op_ffi_call_ptr::decl::<P>(),
      op_ffi_call_ptr_nonblocking::decl::<P>(),
      op_ffi_ptr_of::decl::<P>(),
      op_ffi_get_buf::decl::<P>(),
      op_ffi_buf_copy_into::decl::<P>(),
      op_ffi_cstr_read::decl::<P>(),
      op_ffi_read_bool::decl::<P>(),
      op_ffi_read_u8::decl::<P>(),
      op_ffi_read_i8::decl::<P>(),
      op_ffi_read_u16::decl::<P>(),
      op_ffi_read_i16::decl::<P>(),
      op_ffi_read_u32::decl::<P>(),
      op_ffi_read_i32::decl::<P>(),
      op_ffi_read_u64::decl::<P>(),
      op_ffi_read_i64::decl::<P>(),
      op_ffi_read_f32::decl::<P>(),
      op_ffi_read_f64::decl::<P>(),
      op_ffi_unsafe_callback_create::decl::<P>(),
      op_ffi_unsafe_callback_ref::decl(),
      op_ffi_unsafe_callback_unref::decl(),
    ])
    .event_loop_middleware(|op_state_rc, _cx| {
      // FFI callbacks coming in from other threads will call in and get queued.
      let mut maybe_scheduling = false;

      let mut work_items: Vec<PendingFfiAsyncWork> = vec![];

      {
        let mut op_state = op_state_rc.borrow_mut();
        let ffi_state = op_state.borrow_mut::<FfiState>();

        while let Ok(Some(async_work_fut)) =
          ffi_state.async_work_receiver.try_next()
        {
          // Move received items to a temporary vector so that we can drop the `op_state` borrow before we do the work.
          work_items.push(async_work_fut);
          maybe_scheduling = true;
        }

        drop(op_state);
      }
      while let Some(async_work_fut) = work_items.pop() {
        async_work_fut();
      }

      maybe_scheduling
    })
    .state(move |state| {
      // Stolen from deno_webgpu, is there a better option?
      state.put(Unstable(unstable));

      let (async_work_sender, async_work_receiver) =
        mpsc::unbounded::<PendingFfiAsyncWork>();

      state.put(FfiState {
        async_work_receiver,
        async_work_sender,
      });

      Ok(())
    })
    .build()
}
