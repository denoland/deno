use crate::isolate::ZeroCopyBuf;
use crate::ops::CoreOp;
use crate::resources::ResourceTable;
use std::cell::RefCell;
use std::rc::Rc;

pub struct PluginState {
  resource_table: Rc<RefCell<ResourceTable>>,
}

impl Clone for PluginState {
  fn clone(&self) -> Self {
    PluginState {
      resource_table: self.resource_table.clone(),
    }
  }
}

impl PluginState {
  pub fn new(resource_table: Rc<RefCell<ResourceTable>>) -> Self {
    Self { resource_table }
  }

  pub fn resource_table(&self) -> Rc<RefCell<ResourceTable>> {
    self.resource_table.clone()
  }
}

pub type PluginInitFn = fn(context: &mut dyn PluginInitContext);

type StatefulOpFn =
  dyn Fn(&PluginState, &[u8], Option<ZeroCopyBuf>) -> CoreOp + 'static;

pub trait PluginInitContext {
  fn register_op(
    &mut self,
    name: &str,
    op: Box<dyn Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp + 'static>,
  );

  fn state(&self) -> PluginState;

  fn stateful_op(
    &self,
    op: Box<StatefulOpFn>,
  ) -> Box<dyn Fn(&[u8], Option<ZeroCopyBuf>) -> CoreOp + 'static> {
    let state = self.state();

    Box::new(
      move |control: &[u8], zero_copy: Option<ZeroCopyBuf>| -> CoreOp {
        op(&state, control, zero_copy)
      },
    )
  }
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
