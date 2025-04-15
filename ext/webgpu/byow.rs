// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::ffi::c_void;
#[cfg(any(
  target_os = "linux",
  target_os = "macos",
  target_os = "freebsd",
  target_os = "openbsd"
))]
use std::ptr::NonNull;

use deno_core::cppgc::SameObject;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::Local;
use deno_core::v8::Value;
use deno_core::FromV8;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_error::JsErrorBox;

use crate::surface::GPUCanvasContext;

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

// TODO(@littledivy): This will extend `OffscreenCanvas` when we add it.
pub struct UnsafeWindowSurface {
  pub id: wgpu_core::id::SurfaceId,
  pub width: RefCell<u32>,
  pub height: RefCell<u32>,

  pub context: SameObject<GPUCanvasContext>,
}

impl GarbageCollected for UnsafeWindowSurface {}

#[op2]
impl UnsafeWindowSurface {
  #[constructor]
  #[cppgc]
  fn new(
    state: &mut OpState,
    #[from_v8] options: UnsafeWindowSurfaceOptions,
  ) -> Result<UnsafeWindowSurface, ByowError> {
    let instance = state
      .try_borrow::<super::Instance>()
      .ok_or(ByowError::WebGPUNotInitiated)?;

    // Security note:
    //
    // The `window_handle` and `display_handle` options are pointers to
    // platform-specific window handles.
    //
    // The code below works under the assumption that:
    //
    // - handles can only be created by the FFI interface which
    // enforces --allow-ffi.
    //
    // - `*const c_void` deserizalizes null and v8::External.
    //
    // - Only FFI can export v8::External to user code.
    if options.window_handle.is_null() {
      return Err(ByowError::InvalidParameters);
    }

    let (win_handle, display_handle) = raw_window(
      options.system,
      options.window_handle,
      options.display_handle,
    )?;

    // SAFETY: see above comment
    let id = unsafe {
      instance
        .instance_create_surface(display_handle, win_handle, None)
        .map_err(ByowError::CreateSurface)?
    };

    Ok(UnsafeWindowSurface {
      id,
      width: RefCell::new(options.width),
      height: RefCell::new(options.height),
      context: SameObject::new(),
    })
  }

  #[global]
  fn get_context(
    &self,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::HandleScope,
  ) -> v8::Global<v8::Object> {
    self.context.get(scope, |_| GPUCanvasContext {
      surface_id: self.id,
      width: self.width.clone(),
      height: self.height.clone(),
      config: RefCell::new(None),
      texture: RefCell::new(None),
      canvas: this,
    })
  }

  #[nofast]
  fn present(&self, scope: &mut v8::HandleScope) -> Result<(), JsErrorBox> {
    let Some(context) = self.context.try_unwrap(scope) else {
      return Err(JsErrorBox::type_error("getContext was never called"));
    };

    context.present().map_err(JsErrorBox::from_err)
  }
}

struct UnsafeWindowSurfaceOptions {
  system: UnsafeWindowSurfaceSystem,
  window_handle: *const c_void,
  display_handle: *const c_void,
  width: u32,
  height: u32,
}

#[derive(Eq, PartialEq)]
enum UnsafeWindowSurfaceSystem {
  Cocoa,
  Win32,
  X11,
  Wayland,
}

impl<'a> FromV8<'a> for UnsafeWindowSurfaceOptions {
  type Error = JsErrorBox;

  fn from_v8(
    scope: &mut v8::HandleScope<'a>,
    value: Local<'a, Value>,
  ) -> Result<Self, Self::Error> {
    let obj = value
      .try_cast::<v8::Object>()
      .map_err(|_| JsErrorBox::type_error("is not an object"))?;

    let key = v8::String::new(scope, "system").unwrap();
    let val = obj
      .get(scope, key.into())
      .ok_or_else(|| JsErrorBox::type_error("missing field 'system'"))?;
    let s = String::from_v8(scope, val).unwrap();
    let system = match s.as_str() {
      "cocoa" => UnsafeWindowSurfaceSystem::Cocoa,
      "win32" => UnsafeWindowSurfaceSystem::Win32,
      "x11" => UnsafeWindowSurfaceSystem::X11,
      "wayland" => UnsafeWindowSurfaceSystem::Wayland,
      _ => {
        return Err(JsErrorBox::type_error(format!(
          "Invalid system kind '{s}'"
        )))
      }
    };

    let key = v8::String::new(scope, "windowHandle").unwrap();
    let val = obj
      .get(scope, key.into())
      .ok_or_else(|| JsErrorBox::type_error("missing field 'windowHandle'"))?;
    let Some(window_handle) = deno_core::_ops::to_external_option(&val) else {
      return Err(JsErrorBox::type_error("expected external"));
    };

    let key = v8::String::new(scope, "displayHandle").unwrap();
    let val = obj
      .get(scope, key.into())
      .ok_or_else(|| JsErrorBox::type_error("missing field 'displayHandle'"))?;
    let Some(display_handle) = deno_core::_ops::to_external_option(&val) else {
      return Err(JsErrorBox::type_error("expected external"));
    };

    let key = v8::String::new(scope, "width").unwrap();
    let val = obj
      .get(scope, key.into())
      .ok_or_else(|| JsErrorBox::type_error("missing field 'width'"))?;
    let width = deno_core::convert::Number::<u32>::from_v8(scope, val)?.0;

    let key = v8::String::new(scope, "height").unwrap();
    let val = obj
      .get(scope, key.into())
      .ok_or_else(|| JsErrorBox::type_error("missing field 'height'"))?;
    let height = deno_core::convert::Number::<u32>::from_v8(scope, val)?.0;

    Ok(Self {
      system,
      window_handle,
      display_handle,
      width,
      height,
    })
  }
}

type RawHandles = (
  raw_window_handle::RawWindowHandle,
  raw_window_handle::RawDisplayHandle,
);

#[cfg(target_os = "macos")]
fn raw_window(
  system: UnsafeWindowSurfaceSystem,
  _ns_window: *const c_void,
  ns_view: *const c_void,
) -> Result<RawHandles, ByowError> {
  if system != UnsafeWindowSurfaceSystem::Cocoa {
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
  system: UnsafeWindowSurfaceSystem,
  window: *const c_void,
  hinstance: *const c_void,
) -> Result<RawHandles, ByowError> {
  use raw_window_handle::WindowsDisplayHandle;
  if system != UnsafeWindowSurfaceSystem::Win32 {
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
  system: UnsafeWindowSurfaceSystem,
  window: *const c_void,
  display: *const c_void,
) -> Result<RawHandles, ByowError> {
  let (win_handle, display_handle);
  if system == UnsafeWindowSurfaceSystem::X11 {
    win_handle = raw_window_handle::RawWindowHandle::Xlib(
      raw_window_handle::XlibWindowHandle::new(window as *mut c_void as _),
    );

    display_handle = raw_window_handle::RawDisplayHandle::Xlib(
      raw_window_handle::XlibDisplayHandle::new(
        NonNull::new(display as *mut c_void),
        0,
      ),
    );
  } else if system == UnsafeWindowSurfaceSystem::Wayland {
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
  _system: UnsafeWindowSurfaceSystem,
  _window: *const c_void,
  _display: *const c_void,
) -> Result<RawHandles, deno_error::JsErrorBox> {
  Err(deno_error::JsErrorBox::type_error("Unsupported platform"))
}
