// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::CoreOpHandler;
#[allow(dead_code)]
use crate::OpId;
use std::collections::HashMap;

#[derive(Default)]
pub struct OpRegistry {
  pub ops: Vec<Box<CoreOpHandler>>,
  pub phone_book: HashMap<String, OpId>,
}

impl OpRegistry {
  #[allow(dead_code)]
  pub fn get_op_map(&self) -> HashMap<String, OpId> {
    self.phone_book.clone()
  }

  pub fn register_op(
    &mut self,
    name: &str,
    serialized_op: Box<CoreOpHandler>,
  ) -> OpId {
    let op_id = self.ops.len() as u32;

    self
      .phone_book
      .entry(name.to_string())
      .and_modify(|_| panic!("Op already registered {}", op_id))
      .or_insert(op_id);

    self.ops.push(serialized_op);
    op_id
  }
}
