use crate::isolate::ZeroCopyBuf;
use crate::ops::CoreOp;

pub type PluginInitFn = fn(context: &mut dyn PluginInitContext);

pub trait PluginInitContext {
  fn register_op(
    &mut self,
    name: &str,
    op: Box<
      dyn Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp + Send + Sync + 'static,
    >,
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
