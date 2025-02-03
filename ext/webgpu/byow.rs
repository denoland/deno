// Copyright 2018-2025 the Deno authors. MIT license.

use std::ffi::c_void;
#[cfg(any(
  target_os = "linux",
  target_os = "macos",
  target_os = "freebsd",
  target_os = "openbsd"
))]
use std::ptr::NonNull;

use deno_core::op2;
use deno_core::OpState;
use deno_core::ResourceId;

use crate::surface::WebGpuSurface;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ByowError {
  #[class(type)]
  #[error("Cannot create surface outside of WebGPU context. Did you forget to call `navigator.gpu.requestAdapter()`?")]
  WebGPUNotInitiated,
  #[class(type)]
  #[error("Invalid parameters")]
  InvalidParameters,
  #[class(generic)]
  #[error(transparent)]
  CreateSurface(wgpu_core::instance::CreateSurfaceError),
  #[cfg(target_os = "windows")]
  #[class(type)]
  #[error("Invalid system on Windows")]
  InvalidSystem,
  #[cfg(target_os = "macos")]
  #[class(type)]
  #[error("Invalid system on macOS")]
  InvalidSystem,
  #[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd"
  ))]
  #[class(type)]
  #[error("Invalid system on Linux/BSD")]
  InvalidSystem,
  #[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd"
  ))]
  #[class(type)]
  #[error("window is null")]
  NullWindow,
  #[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd"
  ))]
  #[class(type)]
  #[error("display is null")]
  NullDisplay,
  #[cfg(target_os = "macos")]
  #[class(type)]
  #[error("ns_view is null")]
  NSViewDisplay,
}

#[op2(fast)]
#[smi]
pub fn op_webgpu_surface_create(
  state: &mut OpState,
  #[string] system: &str,
  p1: *const c_void,
  p2: *const c_void,
) -> Result<ResourceId, ByowError> {
  let instance = state
    .try_borrow::<super::Instance>()
    .ok_or(ByowError::WebGPUNotInitiated)?;
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
    return Err(ByowError::InvalidParameters);
  }

  let (win_handle, display_handle) = raw_window(system, p1, p2)?;
  // SAFETY: see above comment
  let surface = unsafe {
    instance
      .instance_create_surface(display_handle, win_handle, None)
      .map_err(ByowError::CreateSurface)?
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
) -> Result<RawHandles, ByowError> {
  if system != "cocoa" {
    return Err(ByowError::InvalidSystem);
  }

  let win_handle = raw_window_handle::RawWindowHandle::AppKit(
    raw_window_handle::AppKitWindowHandle::new(
      NonNull::new(ns_view as *mut c_void).ok_or(ByowError::NSViewDisplay)?,
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
) -> Result<RawHandles, ByowError> {
  use raw_window_handle::WindowsDisplayHandle;
  if system != "win32" {
    return Err(ByowError::InvalidSystem);
  }

  let win_handle = {
    let mut handle = raw_window_handle::Win32WindowHandle::new(
      std::num::NonZeroIsize::new(window as isize)
        .ok_or(ByowError::NullWindow)?,
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
) -> Result<RawHandles, ByowError> {
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
        NonNull::new(window as *mut c_void).ok_or(ByowError::NullWindow)?,
      ),
    );

    display_handle = raw_window_handle::RawDisplayHandle::Wayland(
      raw_window_handle::WaylandDisplayHandle::new(
        NonNull::new(display as *mut c_void).ok_or(ByowError::NullDisplay)?,
      ),
    );
  } else {
    return Err(ByowError::InvalidSystem);
  }

  Ok((win_handle, display_handle))
}

#[cfg(not(any(
  target_os = "macos",
  target_os = "windows",
  target_os = "linux",
  target_os = "freebsd",
  target_os = "openbsd",
)))]
fn raw_window(
  _system: &str,
  _window: *const c_void,
  _display: *const c_void,
) -> Result<RawHandles, deno_error::JsErrorBox> {
  Err(deno_error::JsErrorBox::type_error("Unsupported platform"))
}
