// Copyright 2018-2026 the Deno authors. MIT license.

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
use deno_webgpu::wgpu_core;
use deno_webgpu::wgpu_types;

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
  #[class(type)]
  #[error("Could not get a wgpu instance")]
  NoWgpuInstance,
}

/// GPU state for presenting a Canvas2D context to this window surface.
/// Created lazily on the first `getContext("2d")` call.
struct Canvas2DPresentState {
  queue_id: wgpu_core::id::QueueId,
  format: wgpu_types::TextureFormat,
}

pub struct UnsafeWindowSurface {
  pub data: Rc<RefCell<SurfaceData>>,
  pub active_context: OnceCell<(String, v8::Global<v8::Value>)>,
  canvas2d_present: RefCell<Option<Canvas2DPresentState>>,
}

impl UnsafeWindowSurface {
  pub fn from_surface_data(data: Rc<RefCell<SurfaceData>>) -> Self {
    Self {
      data,
      active_context: Default::default(),
      canvas2d_present: RefCell::new(None),
    }
  }
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
    self.data.borrow_mut().width = value;

    if let Some((id, active_context)) = self.active_context.get() {
      let active_context = v8::Local::new(scope, active_context);
      match get_context(id, scope, active_context) {
        Context::Bitmap(context) => context.resize()?,
        Context::Canvas2D(context) => {
          context.resize(value, self.data.borrow().height)
        }
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
    self.data.borrow_mut().height = value;

    if let Some((id, active_context)) = self.active_context.get() {
      let active_context = v8::Local::new(scope, active_context);
      match get_context(id, scope, active_context) {
        Context::Bitmap(context) => context.resize()?,
        Context::Canvas2D(context) => {
          context.resize(self.data.borrow().width, value)
        }
        Context::WebGPU(context) => context.resize(scope),
      }
    }

    Ok(())
  }

  #[constructor]
  #[cppgc]
  fn new(
    state: &mut OpState,
    #[scoped] options: UnsafeWindowSurfaceOptions,
  ) -> Result<UnsafeWindowSurface, ByowError> {
    let (_, instance) = deno_webgpu::get_or_init_instance(
      state,
      &deno_webgpu::adapter::GPURequestAdapterOptions {
        feature_level: "core".to_string(),
        power_preference: None,
        force_fallback_adapter: false,
      },
    )
    .ok_or(ByowError::NoWgpuInstance)?;

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
        instance,
      })),
      active_context: Default::default(),
      canvas2d_present: RefCell::new(None),
    })
  }

  fn get_context<'s>(
    &self,
    state: Rc<RefCell<OpState>>,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::PinScope<'s, '_>,
    #[webidl] context_id: String,
    #[webidl] options: v8::Local<'s, v8::Value>,
  ) -> Result<Option<v8::Global<v8::Value>>, JsErrorBox> {
    if self.active_context.get().is_none() {
      let instance = state
        .borrow()
        .try_borrow::<deno_webgpu::Instance>()
        .expect("accessed in constructor")
        .clone();

      let context = match context_id.as_str() {
        deno_web::canvas2d::CONTEXT_ID => {
          let ctx = deno_web::canvas2d::create_context(
            state.clone(),
            Some(instance.clone()),
            this,
            ContextData::Surface(self.data.clone()),
            scope,
            options,
            "Failed to execute 'getContext' on 'UnsafeWindowSurface'",
            "Argument 2",
          )?;
          // Initialize the wgpu_core device for Canvas2D surface presentation.
          let (width, height) = {
            let d = self.data.borrow();
            (d.width, d.height)
          };
          let ps = init_canvas2d_present_state(
            &instance,
            self.data.borrow().id,
            width,
            height,
            wgpu_types::Backends::all(),
          )?;
          *self.canvas2d_present.borrow_mut() = Some(ps);
          ctx
        }
        _ => {
          let create_context: CreateCanvasContext =
            match context_id.as_str() {
              super::bitmaprenderer::CONTEXT_ID => {
                super::bitmaprenderer::create as _
              }
              deno_webgpu::canvas::CONTEXT_ID => {
                deno_webgpu::canvas::create as _
              }
              _ => return Ok(None),
            };
          create_context(
            state,
            Some(instance),
            this,
            ContextData::Surface(self.data.clone()),
            scope,
            options,
            "Failed to execute 'getContext' on 'UnsafeWindowSurface'",
            "Argument 2",
          )?
        }
      };

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
      Context::Canvas2D(context) => {
        let present_state = self.canvas2d_present.borrow();
        let Some(ps) = present_state.as_ref() else {
          return Err(JsErrorBox::type_error(
            "Canvas2D surface not initialized for presentation",
          ));
        };
        let data = self.data.borrow();

        // Render the accumulated scene to raw RGBA8 bytes.
        let mut bytes = context
          .render_to_bytes()
          .map_err(|e| JsErrorBox::generic(format!("canvas2d render: {e}")))?;

        // Convert RGBA8 to BGRA8 if the surface format requires it.
        if ps.format == wgpu_types::TextureFormat::Bgra8Unorm
          || ps.format == wgpu_types::TextureFormat::Bgra8UnormSrgb
        {
          for chunk in bytes.chunks_exact_mut(4) {
            chunk.swap(0, 2); // R ↔ B
          }
        }

        // Acquire the current surface texture.
        let surface_output = data
          .instance
          .surface_get_current_texture(data.id, None)
          .map_err(|e| {
            JsErrorBox::generic(format!("canvas2d surface texture: {e}"))
          })?;
        let texture_id = surface_output.texture.ok_or_else(|| {
          JsErrorBox::generic("canvas2d: no surface texture available")
        })?;

        // Upload rendered bytes directly to the surface texture.
        data
          .instance
          .queue_write_texture(
            ps.queue_id,
            &wgpu_types::TexelCopyTextureInfo {
              texture: texture_id,
              mip_level: 0,
              origin: wgpu_types::Origin3d::ZERO,
              aspect: wgpu_types::TextureAspect::All,
            },
            &bytes,
            &wgpu_types::TexelCopyBufferLayout {
              offset: 0,
              bytes_per_row: Some(data.width * 4),
              rows_per_image: None,
            },
            &wgpu_types::Extent3d {
              width: data.width,
              height: data.height,
              depth_or_array_layers: 1,
            },
          )
          .map_err(|e| {
            JsErrorBox::generic(format!("canvas2d write texture: {e}"))
          })?;

        // Submit an empty command list to flush the write before present.
        let no_commands: &[wgpu_core::id::CommandBufferId] = &[];
        if let Err((_, e)) =
          data.instance.queue_submit(ps.queue_id, no_commands)
        {
          return Err(JsErrorBox::generic(format!(
            "canvas2d queue submit: {e:?}"
          )));
        }

        data
          .instance
          .surface_present(data.id)
          .map_err(|e| JsErrorBox::generic(e.to_string()))?;
      }
    }

    Ok(())
  }
}

impl UnsafeWindowSurface {}

/// Initializes a wgpu_core device/queue for presenting Canvas2D to a window surface.
///
/// Requests an adapter compatible with the surface, creates a device, and
/// configures the surface with `COPY_DST` usage so that rendered bytes can
/// be uploaded directly via `queue_write_texture`.
fn init_canvas2d_present_state(
  instance: &deno_webgpu::Instance,
  surface_id: wgpu_core::id::SurfaceId,
  width: u32,
  height: u32,
  backends: wgpu_types::Backends,
) -> Result<Canvas2DPresentState, JsErrorBox> {
  // Request an adapter that is compatible with the window surface.
  let adapter_id = instance
    .request_adapter(
      &wgpu_types::RequestAdapterOptions {
        compatible_surface: Some(surface_id),
        power_preference: wgpu_types::PowerPreference::None,
        force_fallback_adapter: false,
      },
      backends,
      None,
    )
    .map_err(|e| {
      JsErrorBox::generic(format!("canvas2d: no compatible adapter: {e}"))
    })?;

  // Create device and queue on the chosen adapter.
  let (device_id, queue_id) = instance
    .adapter_request_device(
      adapter_id,
      &wgpu_types::DeviceDescriptor::default(),
      None,
      None,
    )
    .map_err(|e| {
      JsErrorBox::generic(format!("canvas2d: device creation failed: {e}"))
    })?;

  // Choose Rgba8Unorm as the surface format (most portable for byte uploads).
  // The present arm converts to Bgra8 if the surface requires it.
  let format = wgpu_types::TextureFormat::Rgba8Unorm;

  // Configure the surface with COPY_DST so queue_write_texture can target it.
  if let Some(err) = instance.surface_configure(
    surface_id,
    device_id,
    &wgpu_types::SurfaceConfiguration {
      usage: wgpu_types::TextureUsages::COPY_DST,
      format,
      width,
      height,
      present_mode: wgpu_types::PresentMode::Fifo,
      view_formats: vec![],
      desired_maximum_frame_latency: 2,
      alpha_mode: wgpu_types::CompositeAlphaMode::Opaque,
    },
  ) {
    return Err(JsErrorBox::generic(format!(
      "canvas2d: surface configure failed: {err}"
    )));
  }

  Ok(Canvas2DPresentState { queue_id, format })
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
    let width = <u32>::from_v8(scope, val).map_err(JsErrorBox::from_err)?;

    let key = v8::String::new(scope, "height").unwrap();
    let val = obj
      .get(scope, key.into())
      .ok_or_else(|| JsErrorBox::type_error("missing field 'height'"))?;
    let height = <u32>::from_v8(scope, val).map_err(JsErrorBox::from_err)?;

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
  Option<raw_window_handle::RawDisplayHandle>,
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
  Ok((win_handle, Some(display_handle)))
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
  Ok((win_handle, Some(display_handle)))
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

  Ok((win_handle, Some(display_handle)))
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
