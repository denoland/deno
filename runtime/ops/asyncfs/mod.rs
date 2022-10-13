#[cfg(feature = "enable_iouring")]
pub mod iouring;
#[cfg(not(feature = "enable_iouring"))]
pub mod spawn_blocking;

#[cfg(feature = "enable_iouring")]
pub use iouring::*;

#[cfg(not(feature = "enable_iouring"))]
pub use spawn_blocking::*;
