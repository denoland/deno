use crate::gotham_state::GothamState;
use std::ops::Deref;
use std::ops::DerefMut;

#[derive(Default)]
pub struct OpState {
  pub resource_table: crate::ResourceTable,
  gotham_state: GothamState,
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
