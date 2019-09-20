// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::CoreOp;
use crate::CoreOpHandler;
use crate::Op;
use crate::OpId;
use crate::PinnedBuf;
use std::collections::HashMap;

#[derive(Default)]
pub struct OpRegistry {
  pub ops: Vec<Box<CoreOpHandler>>,
  pub phone_book: HashMap<String, OpId>,
}

fn get_op_map(_control: &[u8], _zero_copy_buf: Option<PinnedBuf>) -> CoreOp {
  Op::Sync(Box::new([]))
}

impl OpRegistry {
  pub fn new() -> Self {
    // TODO: this is make shift fix for get op map
    let mut registry = Self::default();
    registry.register_op("get_op_map", Box::new(get_op_map));
    registry
  }

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
