// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::OnceCell;
use std::cell::RefCell;
use std::ffi::c_void;
#[cfg(any(
  target_os = "linux",
  target_os = "macos",
  target_os = "freebsd",
  target_os = "openbsd"
))]
use std::ptr::NonNull;
use std::rc::Rc;

use deno_core::FromV8;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_webgpu::canvas::ContextData;
use deno_webgpu::canvas::SurfaceData;

use crate::canvas::Context;
use crate::canvas::CreateCanvasContext;
use crate::canvas::get_context;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ByowError {
  #[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
  )))]
  #[class(type)]
  #[error("Unsupported platform")]
  Unsupported,
  #[class(type)]
  #[error(
    "Cannot create surface outside of WebGPU context. Did you forget to call `navigator.gpu.requestAdapter()`?"
  )]
  WebGPUNotInitiated,
  #[class(type)]
  #[error("Invalid parameters")]
  InvalidParameters,
  #[class(generic)]
  #[error(transparent)]
  CreateSurface(deno_webgpu::wgpu_core::instance::CreateSurfaceError),
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

pub struct UnsafeWindowSurface {
  pub data: Rc<RefCell<SurfaceData>>,

  pub active_context: OnceCell<(String, v8::Global<v8::Value>)>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for UnsafeWindowSurface {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"UnsafeWindowSurface"
  }
}

#[op2]
impl UnsafeWindowSurface {
  #[getter]
  fn width(&self) -> u32 {
    let data = self.data.borrow();
    data.width
  }
  #[setter]
  fn width(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    value: u32,
  ) -> Result<(), JsErrorBox> {
    let mut data = self.data.borrow_mut();
    data.width = value;

    if let Some((id, active_context)) = self.active_context.get() {
      let active_context = v8::Local::new(scope, active_context);
      match get_context(id, scope, active_context) {
        Context::Bitmap(context) => context.resize()?,
        Context::WebGPU(context) => context.resize(scope),
      }
    }

    Ok(())
  }

  #[getter]
  fn height(&self) -> u32 {
    let data = self.data.borrow();
    data.height
  }
  #[setter]
  fn height(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    value: u32,
  ) -> Result<(), JsErrorBox> {
    let mut data = self.data.borrow_mut();
    data.height = value;

    if let Some((id, active_context)) = self.active_context.get() {
      let active_context = v8::Local::new(scope, active_context);
      match get_context(id, scope, active_context) {
        Context::Bitmap(context) => context.resize()?,
        Context::WebGPU(context) => context.resize(scope),
      }
    }

    Ok(())
  }

  #[constructor]
  #[cppgc]
  fn new(
    state: &mut OpState,
    #[from_v8] options: UnsafeWindowSurfaceOptions,
  ) -> Result<UnsafeWindowSurface, ByowError> {
    let instance = state
      .try_borrow::<deno_webgpu::Instance>()
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
      data: Rc::new(RefCell::new(SurfaceData {
        width: options.width,
        height: options.height,
        id,
      })),
      active_context: Default::default(),
    })
  }

  #[global]
  fn get_context<'s>(
    &self,
    state: &mut OpState,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'s, '_>,
    #[webidl] context_id: String,
    #[webidl] options: v8::Local<'s, v8::Value>,
  ) -> Result<Option<v8::Global<v8::Value>>, JsErrorBox> {
    if self.active_context.get().is_none() {
      let create_context: CreateCanvasContext = match context_id.as_str() {
        super::bitmaprenderer::CONTEXT_ID => super::bitmaprenderer::create as _,
        deno_webgpu::canvas::CONTEXT_ID => deno_webgpu::canvas::create as _,
        _ => {
          return Err(JsErrorBox::new(
            "DOMExceptionNotSupportedError",
            format!("Context '{context_id}' not implemented"),
          ));
        }
      };

      let instance = state
        .try_borrow::<deno_webgpu::Instance>()
        .expect("accessed in constructor")
        .clone();

      let context = create_context(
        Some(instance),
        this,
        ContextData::Surface(self.data.clone()),
        scope,
        options,
        "Failed to execute 'getContext' on 'OffscreenCanvas'",
        "Argument 2",
      )?;
      let _ = self.active_context.set((context_id.clone(), context));
    }

    let (name, context) = self.active_context.get().unwrap();

    if &context_id == name {
      Ok(Some(context.clone()))
    } else {
      Ok(None)
    }
  }

  #[nofast]
  fn present(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
  ) -> Result<(), JsErrorBox> {
    let Some(active_context) = self.active_context.get() else {
      return Err(JsErrorBox::new(
        "DOMExceptionInvalidStateError",
        "UnsafeWindowSurface hasn't been initialized yet",
      ));
    };

    let active_context_local = v8::Local::new(scope, &active_context.1);
    let context = get_context(&active_context.0, scope, active_context_local);
    match &context {
      Context::Bitmap(context) => {
        let data = self.data.borrow();

        let super::bitmaprenderer::SurfaceBitmap { instance, .. } =
          context.surface_only.as_ref().unwrap();

        instance
          .surface_present(data.id)
          .map_err(|e| JsErrorBox::generic(e.to_string()))?;
      }
      Context::WebGPU(context) => {
        let configuration = context.configuration.borrow();
        let configuration = configuration.as_ref().ok_or_else(|| {
          JsErrorBox::type_error("GPUCanvasContext has not been configured")
        })?;

        let data = self.data.borrow();

        configuration
          .device
          .instance
          .surface_present(data.id)
          .map_err(|e| JsErrorBox::generic(e.to_string()))?;

        // next `get_current_texture` call would get a new texture
        *context.current_texture.borrow_mut() = None;
      }
    }

    Ok(())
  }
}

impl UnsafeWindowSurface {}

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
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
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
        )));
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
    let width = deno_core::convert::Number::<u32>::from_v8(scope, val)
      .map_err(JsErrorBox::from_err)?
      .0;

    let key = v8::String::new(scope, "height").unwrap();
    let val = obj
      .get(scope, key.into())
      .ok_or_else(|| JsErrorBox::type_error("missing field 'height'"))?;
    let height = deno_core::convert::Number::<u32>::from_v8(scope, val)
      .map_err(JsErrorBox::from_err)?
      .0;

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
) -> Result<RawHandles, ByowError> {
  Err(ByowError::Unsupported)
}
