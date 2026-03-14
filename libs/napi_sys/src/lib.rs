// Copyright 2018-2026 the Deno authors. MIT license.

// Forked from napi-sys (https://github.com/napi-rs/napi-rs)
// to avoid build-time dependency on Node.js binaries and to allow
// adding new Node-API symbols in-tree.
//
// All napi version feature gates have been removed — everything is
// always available.

#[cfg(windows)]
macro_rules! generate {
  (extern "C" {
      $(fn $name:ident($($param:ident: $ptype:ty$(,)?)*)$( -> $rtype:ty)?;)+
  }) => {
      struct Napi {
          $(
              $name: unsafe extern "C" fn(
                  $($param: $ptype,)*
              )$( -> $rtype)*,
          )*
      }

      #[inline(never)]
      fn panic_load<T>() -> T {
          panic!("Must load N-API bindings")
      }

      static mut NAPI: Napi = {
          $(
              unsafe extern "C" fn $name($(_: $ptype,)*)$( -> $rtype)* {
                  panic_load()
              }
          )*

          Napi {
              $(
                  $name,
              )*
          }
      };

      /// Loads N-API symbols from the given host library into the
      /// global function table.
      ///
      /// # Safety
      ///
      /// Must only be called once from a single thread (e.g. via
      /// [`setup`]). The host library must export valid N-API symbols.
      pub unsafe fn load(
          host: &libloading::Library,
      ) -> Result<(), libloading::Error> {
          // SAFETY: this function is only called from setup() which
          // uses Once to ensure single-threaded initialization.
          unsafe {
            $(
              {
                let symbol: Result<libloading::Symbol<unsafe extern "C" fn ($(_: $ptype,)*)$( -> $rtype)*>, libloading::Error> = host.get(stringify!($name).as_bytes());
                match symbol {
                  Ok(f) => NAPI.$name = *f,
                  Err(_e) => {
                    debug_assert!({
                      println!("Load Node-API [{}] from host runtime failed: {}", stringify!($name), _e);
                      true
                    });
                  }
                }
              }
            )*
          }

          Ok(())
      }

      $(
          /// # Safety
          ///
          /// Caller must ensure NAPI has been initialized via [`setup`].
          #[inline]
          pub unsafe fn $name($($param: $ptype,)*)$( -> $rtype)* {
              // SAFETY: caller must ensure NAPI has been initialized via setup().
              unsafe { (NAPI.$name)($($param,)*) }
          }
      )*
  };
}

#[cfg(not(windows))]
macro_rules! generate {
  (extern "C" {
    $(fn $name:ident($($param:ident: $ptype:ty$(,)?)*)$( -> $rtype:ty)?;)+
  }) => {
    unsafe extern "C" {
      $(
        pub safe fn $name($($param: $ptype,)*)$( -> $rtype)*;
      ) *
    }
  };
}

mod functions;
mod types;

#[cfg(windows)]
use std::sync::Once;

pub use functions::*;
pub use types::*;

#[cfg(windows)]
static SETUP: Once = Once::new();

/// Loads N-API symbols from host process.
/// Must be called at least once before using any functions in bindings or
/// they will panic.
///
/// # Safety
///
/// `env` must be a valid `napi_env` for the current thread.
#[cfg(windows)]
pub unsafe fn setup() {
  SETUP.call_once(|| {
    let host = match libloading::os::windows::Library::this() {
      Ok(lib) => lib.into(),
      Err(err) => {
        panic!("Initialize libloading failed {}", err);
      }
    };

    // SAFETY: Once ensures single-threaded init; host is a valid library handle.
    unsafe {
      if let Err(err) = functions::load(&host) {
        panic!("{}", err);
      }
    }
  });
}
