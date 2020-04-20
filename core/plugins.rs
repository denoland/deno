// TODO(ry) This plugin module is superfluous. Try to remove definitions for
// "init_fn!", "PluginInitFn", and "PluginInitContext".

use crate::ops::OpDispatcher;

pub type PluginInitFn = fn(context: &mut dyn PluginInitContext);

pub trait PluginInitContext {
  fn register_op(
    &mut self,
    name: &str,
    op: Box<OpDispatcher>, // TODO(ry) rename to dispatcher, not op.
  );
}

#[macro_export]
macro_rules! init_fn {
  ($fn:path) => {
    #[no_mangle]
    pub fn deno_plugin_init(context: &mut dyn PluginInitContext) {
      $fn(context)
    }
  };
}
