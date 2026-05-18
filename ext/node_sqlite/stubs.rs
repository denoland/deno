// Copyright 2018-2026 the Deno authors. MIT license.
//
// QuickJS stub for deno_node_sqlite. The full v8-tied implementation in
// `database.rs` & friends doesn't compile under qjs_v8_compat (see
// pervasive use of `transmute(NonNull<v8::T>) -> Local<v8::T>` and
// borrow-checker patterns that require V8's exact lifetime relationships).
// Until those are unwound, expose ops that error out so the runtime can
// still wire up `deno_node_sqlite::init()` without crashing.

use deno_core::OpState;
use deno_core::v8;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("node_sqlite is not available in this build")]
pub struct UnsupportedError;

macro_rules! gc_stub {
  ($($name:ident),* $(,)?) => { $(
    pub struct $name;
    unsafe impl deno_core::v8::cppgc::GarbageCollected for $name {
      fn trace(&self, _v: &mut deno_core::v8::cppgc::Visitor) {}
    }
    #[deno_core::op2]
    impl $name {}
  )* };
}
gc_stub!(DatabaseSync, DatabaseSyncLimits, Session, SQLTagStore, StatementSync);

#[deno_core::op2(fast)]
pub fn op_node_database_backup(
  _state: &mut OpState,
) -> Result<(), UnsupportedError> {
  Err(UnsupportedError)
}

deno_core::extension!(
  deno_node_sqlite,
  ops = [op_node_database_backup,],
  objects = [
    DatabaseSync,
    DatabaseSyncLimits,
    Session,
    SQLTagStore,
    StatementSync,
  ],
);
