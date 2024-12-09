// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::ops::Deref;
use std::rc::Rc;

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseSyncOptions {
  open: bool,
  enable_foreign_key_constraints: bool,
}

pub struct DatabaseSync {}

impl GarbageCollected for DatabaseSync {}

#[op2]
impl DatabaseSync {
  #[constructor]
  #[cppgc]
  fn new(
    #[string] location: &str,
    #[serde] options: DatabaseSyncOptions,
  ) -> DatabaseSync {
    DatabaseSync {}
  }

  #[fast]
  fn open(&self) {}

  #[fast]
  fn close(&self) {}

  #[cppgc]
  fn prepare(&self, #[string] sql: &str) -> StatementSync {
    StatementSync {}
  }

  // fn exec() <-- varargs
}

pub struct StatementSync {}

impl GarbageCollected for StatementSync {}

#[op2]
impl StatementSync {
  #[constructor]
  #[cppgc]
  fn new(_: bool) -> StatementSync {
    StatementSync {}
  }

  // fn get() <-- varargs

  #[fast]
  fn run(&self) {}

  #[fast]
  fn all(&self) {}

  #[fast]
  fn set_allowed_bare_named_parameters(&self, enabled: bool) {}

  #[fast]
  fn set_read_bigints(&self, enabled: bool) {}

  #[fast]
  fn source_sql(&self) {}

  #[string]
  fn expanded_sqlite(&self) -> String {
    todo!()
  }
}
