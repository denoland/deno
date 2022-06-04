#[cfg(not(feature = "snaphack"))]
mod compiled;
#[cfg(not(feature = "snaphack"))]
pub use compiled::*;
