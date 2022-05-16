// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use rusqlite::CachedStatement;
use rusqlite::Connection;
use std::borrow::Cow;
use std::path::PathBuf;
use std::cell::RefCell;

pub use rusqlite;

pub fn init() -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/sqlite",
      "01_sqlite.js",
    ))
    .ops(vec![
      op_sqlite_open::decl(),
      op_sqlite_prepare::decl(),
      op_sqlite_run::decl(),
      op_sqlite_query::decl(),
    ])
    .build()
}

pub struct ConnResource {
  conn: Connection,
}

impl Resource for ConnResource {
  fn name(&self) -> Cow<str> {
    "connResource".into()
  }
}

pub struct StmtResource(RefCell<CachedStatement<'static>>);

impl Resource for StmtResource {
  fn name(&self) -> Cow<str> {
    "stmtResource".into()
  }
}

#[op]
fn op_sqlite_open(
  state: &mut OpState,
  path: String,
) -> Result<ResourceId, AnyError> {
  let conn = Connection::open(&path)?;
  let handle = state.resource_table.add(ConnResource {
    conn,
  });
  Ok(handle)
}

#[op]
fn op_sqlite_prepare(
  state: &mut OpState,
  handle: ResourceId,
  sql: String,
) -> Result<ResourceId, AnyError> {
  let resource = Box::leak(Box::new(state.resource_table.get::<ConnResource>(handle)?));
  let stmt = resource.conn.prepare_cached(&sql)?;
  let rid = state.resource_table.add(StmtResource(RefCell::new(stmt)));
  Ok(rid)
}

#[op(v8)]
fn op_sqlite_run(
  scope: &mut v8::HandleScope,
  state: &mut OpState,
  stmt: ResourceId,
  args: Vec<serde_v8::Value>,
) -> Result<usize, AnyError> {
  let stmt = state.resource_table.get::<StmtResource>(stmt)?;
  let mut stmt = stmt.0.borrow_mut();
  for (index, value) in args.into_iter().enumerate() {
    let value = value.v8_value;
    if value.is_null() {
      // stmt.raw_bind_parameter(index, ())?;
    } else if value.is_boolean() {
      stmt.raw_bind_parameter(index, value.is_true())?;
    } else if value.is_int32() {
      stmt.raw_bind_parameter(index, value.integer_value(scope).unwrap())?;
    } else if value.is_number() {
      stmt.raw_bind_parameter(index, value.number_value(scope).unwrap())?;
    } else if value.is_big_int() {
      let bigint = value.to_big_int(scope).unwrap();
      let (value, _) = bigint.i64_value();
      stmt.raw_bind_parameter(index, value)?;
    } else if value.is_string() {
      stmt.raw_bind_parameter(index, value.to_rust_string_lossy(scope))?;
    }
    // TODO: Blobs
  }
  Ok(stmt.raw_execute()?)
}

#[op(v8)]
fn op_sqlite_query(
  scope: &mut v8::HandleScope,
  state: &mut OpState,
  stmt: ResourceId,
  args: Vec<serde_v8::Value>,
) -> Result<(), AnyError> {
  let stmt = state.resource_table.get::<StmtResource>(stmt)?;
  let mut stmt = stmt.0.borrow_mut();
  for (index, value) in args.into_iter().enumerate() {
    let value = value.v8_value;
    if value.is_null() {
      // stmt.raw_bind_parameter(index, ())?;
    } else if value.is_boolean() {
      stmt.raw_bind_parameter(index, value.is_true())?;
    } else if value.is_int32() {
      stmt.raw_bind_parameter(index, value.integer_value(scope).unwrap())?;
    } else if value.is_number() {
      stmt.raw_bind_parameter(index, value.number_value(scope).unwrap())?;
    } else if value.is_big_int() {
      let bigint = value.to_big_int(scope).unwrap();
      let (value, _) = bigint.i64_value();
      stmt.raw_bind_parameter(index, value)?;
    } else if value.is_string() {
      stmt.raw_bind_parameter(index, value.to_rust_string_lossy(scope))?;
    }
    // TODO: Blobs
  }
  stmt.raw_query();
  Ok(())
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_sqlite.d.ts")
}
