#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(ambiguous_glob_reexports)]

// borrowed from https://github.com/neon-bindings/neon/tree/main/crates/neon/src/sys/bindings

#[cfg(feature = "dyn-symbols")]
macro_rules! generate {
  (extern "C" {
    $(fn $name:ident($($param:ident: $ptype:ty$(,)?)*)$( -> $rtype:ty)?;)+
  }) => {
    struct LibUv {
      $(
        $name: unsafe extern "C" fn(
          $($param: $ptype,)*
        )$( -> $rtype)*,
      )*
    }

    #[inline(never)]
    fn panic_load<T>() -> T {
      panic!("Node-API symbol has not been loaded")
    }

    static mut LIBUV: LibUv = {
      $(
        unsafe extern "C" fn $name($(_: $ptype,)*)$( -> $rtype)* {
          panic_load()
        }
      )*

      LibUv {
        $(
          $name,
        )*
      }
    };

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn load(
      host: &libloading::Library,
    ) -> Result<(), libloading::Error> {
      LIBUV = LibUv {
        $(
          $name: {
            let symbol: Result<libloading::Symbol<unsafe extern "C" fn ($(_: $ptype,)*)$( -> $rtype)*>, libloading::Error> = host.get(stringify!($name).as_bytes());
            match symbol {
              Ok(f) => *f,
              #[cfg_attr(not(feature = "warn-missing"), allow(unused_variables))]
              Err(e) => {
                #[cfg(feature = "warn-missing")] {
                  eprintln!("Load libuv [{}] from host failed: {}", stringify!($name), e);
                }
                LIBUV.$name
              }
            }
          },
        )*
      };

      Ok(())
    }

    $(
      #[inline]
      #[allow(clippy::missing_safety_doc)]
      pub unsafe fn $name($($param: $ptype,)*)$( -> $rtype)* {
        (LIBUV.$name)($($param,)*)
      }
    )*
  };
}

#[cfg(not(feature = "dyn-symbols"))]
macro_rules! generate {
  (extern "C" {
    $(fn $name:ident($($param:ident: $ptype:ty$(,)?)*)$( -> $rtype:ty)?;)+
  }) => {
    extern "C" {
      $(
        pub fn $name($($param: $ptype,)*)$( -> $rtype)*;
      ) *
    }
  };
}

pub(crate) use generate;

mod functions;
mod types {
  include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
//
pub use functions::*;
pub use types::*;

/// Loads libuv symbols from host process.
/// Must be called at least once before using any functions in bindings or
/// they will panic.
#[cfg(feature = "dyn-symbols")]
#[allow(clippy::missing_safety_doc)]
pub unsafe fn setup() -> libloading::Library {
  match load_all() {
    Err(err) => panic!("{}", err),
    Ok(l) => l,
  }
}
