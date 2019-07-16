use crate::isolate::CoreOp;
use crate::libdeno::PinnedBuf;

/// Funciton type for plugin ops
pub type PluginDispatchFn =
  fn(data: &[u8], zero_copy: Option<PinnedBuf>) -> CoreOp;

#[macro_export]
macro_rules! declare_plugin_op {
  ($name:ident, $fn:path) => {
    #[no_mangle]
    pub fn $name(data: &[u8], zero_copy: Option<PinnedBuf>) -> CoreOp {
      $fn(data, zero_copy)
    }
  };
}
