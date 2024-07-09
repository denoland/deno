// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::ResourceId;
use std::ffi::c_void;
#[cfg(any(
  target_os = "linux",
  target_os = "macos",
  target_os = "freebsd",
  target_os = "openbsd"
))]
use std::ptr::NonNull;

use crate::surface::WebGpuSurface;

#[op2(fast)]
#[smi]
pub fn op_webgpu_surface_create(
  state: &mut OpState,
  #[string] system: &str,
  p1: *const c_void,
  p2: *const c_void,
) -> Result<ResourceId, AnyError> {
  let instance = state.try_borrow::<super::Instance>().ok_or_else(|| {
    type_error("Cannot create surface outside of WebGPU context. Did you forget to call `navigator.gpu.requestAdapter()`?")
  })?;
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
  // - Only FFI can export v8::External to user code.
  if p1.is_null() {
    return Err(type_error("Invalid parameters"));
  }

  let (win_handle, display_handle) = raw_window(system, p1, p2)?;
  // SAFETY: see above comment
  let surface = unsafe {
    instance.instance_create_surface(display_handle, win_handle, None)?
  };

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
  _ns_window: *const c_void,
  ns_view: *const c_void,
) -> Result<RawHandles, AnyError> {
  if system != "cocoa" {
    return Err(type_error("Invalid system on macOS"));
  }

  let win_handle = raw_window_handle::RawWindowHandle::AppKit(
    raw_window_handle::AppKitWindowHandle::new(
      NonNull::new(ns_view as *mut c_void)
        .ok_or(type_error("ns_view is null"))?,
    ),
  );

  let display_handle = raw_window_handle::RawDisplayHandle::AppKit(
    raw_window_handle::AppKitDisplayHandle::new(),
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
    let mut handle = raw_window_handle::Win32WindowHandle::new(
      std::num::NonZeroIsize::new(window as isize)
        .ok_or(type_error("window is null"))?,
    );
    handle.hinstance = std::num::NonZeroIsize::new(hinstance as isize);

    raw_window_handle::RawWindowHandle::Win32(handle)
  };

  let display_handle =
    raw_window_handle::RawDisplayHandle::Windows(WindowsDisplayHandle::new());
  Ok((win_handle, display_handle))
}

#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
fn raw_window(
  system: &str,
  window: *const c_void,
  display: *const c_void,
) -> Result<RawHandles, AnyError> {
  let (win_handle, display_handle);
  if system == "x11" {
    win_handle = raw_window_handle::RawWindowHandle::Xlib(
      raw_window_handle::XlibWindowHandle::new(window as *mut c_void as _),
    );

    display_handle = raw_window_handle::RawDisplayHandle::Xlib(
      raw_window_handle::XlibDisplayHandle::new(
        NonNull::new(display as *mut c_void),
        0,
      ),
    );
  } else if system == "wayland" {
    win_handle = raw_window_handle::RawWindowHandle::Wayland(
      raw_window_handle::WaylandWindowHandle::new(
        NonNull::new(window as *mut c_void)
          .ok_or(type_error("window is null"))?,
      ),
    );

    display_handle = raw_window_handle::RawDisplayHandle::Wayland(
      raw_window_handle::WaylandDisplayHandle::new(
        NonNull::new(display as *mut c_void)
          .ok_or(type_error("display is null"))?,
      ),
    );
  } else {
    return Err(type_error("Invalid system on Linux/BSD"));
  }

  Ok((win_handle, display_handle))
}
