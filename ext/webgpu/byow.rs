// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::ResourceId;
use std::ffi::c_void;

use crate::surface::WebGpuSurface;

#[op2(fast)]
#[smi]
pub fn op_webgpu_surface_create(
  state: &mut OpState,
  #[string] system: &str,
  p1: *const c_void,
  p2: *const c_void,
) -> Result<ResourceId, AnyError> {
  let instance = state.borrow::<super::Instance>();
  // Security note:
  //
  // The `p1` and `p2` parameters are pointers to platform-specific window
  // handles.
  //
  // The code below works under the assumption that:
  //
  // - handles can only be created by the FFI interface which
  // enforces --allow-ffi.
  //
  // - `*const c_void` deserizalizes null and v8::External.
  //
  // - Only FFI can create v8::External.
  if p1.is_null() {
    return Err(type_error("Invalid parameters"));
  }

  let (win_handle, display_handle) = raw_window(system, p1, p2)?;
  let surface = instance.instance_create_surface(
    display_handle,
    win_handle,
    Default::default(),
  );

  let rid = state
    .resource_table
    .add(WebGpuSurface(instance.clone(), surface));
  Ok(rid)
}

type RawHandles = (
  raw_window_handle::RawWindowHandle,
  raw_window_handle::RawDisplayHandle,
);

#[cfg(target_os = "macos")]
fn raw_window(
  system: &str,
  ns_window: *const c_void,
  ns_view: *const c_void,
) -> Result<RawHandles, AnyError> {
  if system != "cocoa" {
    return Err(type_error("Invalid system on macOS"));
  }

  let win_handle = {
    let mut handle = raw_window_handle::AppKitWindowHandle::empty();
    handle.ns_window = ns_window as *mut c_void;
    handle.ns_view = ns_view as *mut c_void;

    raw_window_handle::RawWindowHandle::AppKit(handle)
  };
  let display_handle = raw_window_handle::RawDisplayHandle::AppKit(
    raw_window_handle::AppKitDisplayHandle::empty(),
  );
  Ok((win_handle, display_handle))
}

#[cfg(target_os = "windows")]
fn raw_window(
  system: &str,
  window: *const c_void,
  hinstance: *const c_void,
) -> Result<RawHandles, AnyError> {
  use raw_window_handle::WindowsDisplayHandle;
  if system != "win32" {
    return Err(type_error("Invalid system on Windows"));
  }

  let win_handle = {
    use raw_window_handle::Win32WindowHandle;

    let mut handle = Win32WindowHandle::empty();
    handle.hwnd = window as *mut c_void;
    handle.hinstance = hinstance as *mut c_void;

    raw_window_handle::RawWindowHandle::Win32(handle)
  };

  let display_handle =
    raw_window_handle::RawDisplayHandle::Windows(WindowsDisplayHandle::empty());
  Ok((win_handle, display_handle))
}

#[cfg(target_os = "linux")]
fn raw_window(
  system: &str,
  window: *const c_void,
  display: *const c_void,
) -> Result<RawHandles, AnyError> {
  if system != "x11" {
    return Err(type_error("Invalid system on Linux"));
  }

  let win_handle = {
    let mut handle = raw_window_handle::XlibWindowHandle::empty();
    handle.window = window as *mut c_void as _;
    handle.display = display as *mut c_void;

    raw_window_handle::RawWindowHandle::Xlib(handle)
  };

  let display_handle = {
    let mut handle = raw_window_handle::XlibDisplayHandle::empty();
    handle.display = display as *mut c_void;

    raw_window_handle::RawDisplayHandle::Xlib(handle)
  };

  Ok((win_handle, display_handle))
}
