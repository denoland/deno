// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

#![allow(unused_mut)]
#![allow(non_camel_case_types)]

//! Symbols to be exported are now defined in this JSON file.
//! The `#[napi_sym]` macro checks for missing entries and panics.
//!
//! `./tools/napi/generate_link_win.js` is used to generate the LINK `cli/exports.def` on Windows,
//! which is also checked into git.
//!
//! To add a new napi function:
//! 1. Place `#[napi_sym]` on top of your implementation.
//! 2. Add the function's identifier to this JSON list.
//! 3. Finally, run `./tools/napi/generate_link_win.js` to update `cli/exports.def`.

pub mod r#async;
pub mod env;
pub mod function;
pub mod js_native_api;
pub mod threadsafe_functions;
pub mod util;

use deno_core::v8;
use std::os::raw::c_int;
use std::os::raw::c_void;

pub type uv_async_t = *mut uv_async;
pub type uv_loop_t = *mut c_void;
pub type uv_async_cb = extern "C" fn(handle: uv_async_t);

use deno_core::futures::channel::mpsc;
#[repr(C)]
pub struct uv_async {
  pub data: Option<*mut c_void>,
  callback: uv_async_cb,
  sender: Option<mpsc::UnboundedSender<deno_core::napi::PendingNapiAsyncWork>>,
  ref_sender:
    Option<mpsc::UnboundedSender<deno_core::napi::ThreadSafeFunctionStatus>>,
}

#[no_mangle]
pub extern "C" fn uv_default_loop() -> uv_loop_t {
  std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn uv_async_init(
  _loop: uv_loop_t,
  async_: uv_async_t,
  cb: uv_async_cb,
) -> c_int {
  unsafe {
    (*async_).callback = cb;
  }
  deno_core::napi::ASYNC_WORK_SENDER.with(|sender| unsafe {
    (*async_).sender = Some(sender.borrow().clone().unwrap());
  });

  deno_core::napi::THREAD_SAFE_FN_SENDER.with(|sender| {
    sender
      .borrow()
      .clone()
      .unwrap()
      .unbounded_send(deno_core::napi::ThreadSafeFunctionStatus::Alive)
      .unwrap();
    unsafe {
      (*async_).ref_sender = Some(sender.borrow().clone().unwrap());
    }
  });

  0
}

#[no_mangle]
pub extern "C" fn uv_async_send(async_: uv_async_t) -> c_int {
  let sender = unsafe { (*async_).sender.as_ref().unwrap() };
  let ref_sender = unsafe { (*async_).ref_sender.as_ref().unwrap() };
  let fut = Box::new(move |_: &mut v8::HandleScope| {
    unsafe { ((*async_).callback)(async_) };
  });

  match sender.unbounded_send(fut) {
    Ok(_) => 0,
    Err(_) => 1,
  }
}
