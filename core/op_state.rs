use crate::gotham_state::GothamState;
use crate::ops::OpTable;
use std::ops::Deref;
use std::ops::DerefMut;

pub struct OpState {
  pub resource_table: crate::ResourceTable,
  pub get_error_class_fn: crate::runtime::GetErrorClassFn,
  pub op_table: OpTable,
  gotham_state: GothamState,
}

impl Default for OpState {
  fn default() -> OpState {
    OpState {
      resource_table: crate::ResourceTable::default(),
      get_error_class_fn: &|_| "Error",
      op_table: OpTable::default(),
      gotham_state: GothamState::default(),
    }
  }
}

impl Deref for OpState {
  type Target = GothamState;

  fn deref(&self) -> &Self::Target {
    &self.gotham_state
  }
}

impl DerefMut for OpState {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.gotham_state
  }
}
