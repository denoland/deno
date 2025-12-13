// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::ffi::c_int;
use std::time;

use deno_core::FromV8;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8_static_strings;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use rusqlite::Connection;
use rusqlite::backup;
use rusqlite::backup::Progress;

use super::DatabaseSync;
use super::SqliteError;
use super::validators;

const DEFAULT_BACKUP_RATE: c_int = 100;

struct BackupOptions<'a> {
  source: String,
  target: String,
  rate: i32,
  progress: Option<v8::Local<'a, v8::Function>>,
}

impl<'a> Default for BackupOptions<'a> {
  fn default() -> Self {
    BackupOptions {
      source: "main".to_string(),
      target: "main".to_string(),
      rate: DEFAULT_BACKUP_RATE,
      progress: None,
    }
  }
}

impl<'a> FromV8<'a> for BackupOptions<'a> {
  type Error = validators::Error;

  fn from_v8(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    use validators::Error;
    let mut options = BackupOptions::default();

    if value.is_undefined() {
      return Ok(options);
    }

    let Ok(obj) = v8::Local::<v8::Object>::try_from(value) else {
      return Err(Error::InvalidArgType(
        "The \"options\" argument must be an object.",
      ));
    };

    v8_static_strings! {
      SOURCE_STRING = "source",
      TARGET_STRING = "target",
      RATE_STRING = "rate",
      PROGRESS_STRING = "progress",
    }

    let source_string = SOURCE_STRING.v8_string(scope).unwrap();
    if let Some(source_val) = obj.get(scope, source_string.into())
      && !source_val.is_undefined()
    {
      let source_str =
        v8::Local::<v8::String>::try_from(source_val).map_err(|_| {
          Error::InvalidArgType(
            "The \"options.source\" argument must be a string.",
          )
        })?;
      options.source = source_str.to_rust_string_lossy(scope);
    }

    let target_string = TARGET_STRING.v8_string(scope).unwrap();
    if let Some(target_val) = obj.get(scope, target_string.into())
      && !target_val.is_undefined()
    {
      let target_str =
        v8::Local::<v8::String>::try_from(target_val).map_err(|_| {
          Error::InvalidArgType(
            "The \"options.target\" argument must be a string.",
          )
        })?;
      options.target = target_str.to_rust_string_lossy(scope);
    }

    let rate_string = RATE_STRING.v8_string(scope).unwrap();
    if let Some(rate_val) = obj.get(scope, rate_string.into())
      && !rate_val.is_undefined()
    {
      let rate_int = v8::Local::<v8::Integer>::try_from(rate_val)
        .map_err(|_| {
          Error::InvalidArgType(
            "The \"options.rate\" argument must be an integer.",
          )
        })?
        .value();

      options.rate = i32::try_from(rate_int).map_err(|_| {
        Error::InvalidArgType(
          "The \"options.rate\" argument must be an integer.",
        )
      })?;
    }

    let progress_string = PROGRESS_STRING.v8_string(scope).unwrap();
    if let Some(progress_val) = obj.get(scope, progress_string.into())
      && !progress_val.is_undefined()
    {
      let progress_fn = v8::Local::<v8::Function>::try_from(progress_val)
        .map_err(|_| {
          Error::InvalidArgType(
            "The \"options.progress\" argument must be a function.",
          )
        })?;
      options.progress = Some(progress_fn);
    }

    Ok(options)
  }
}

#[op2(stack_trace)]
#[smi]
pub fn op_node_database_backup(
  state: &mut OpState,
  #[cppgc] source_db: &DatabaseSync,
  #[string] path: &str,
  #[from_v8] options: BackupOptions,
) -> Result<i32, SqliteError> {
  let src_conn_ref = source_db.conn.borrow();
  let src_conn = src_conn_ref.as_ref().ok_or(SqliteError::SessionClosed)?;
  let path = std::path::Path::new(path);
  let checked_path = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(path),
    OpenAccessKind::Write,
    Some("node:sqlite.backup"),
  )?;

  let mut dst_conn = Connection::open(checked_path)?;
  let backup = backup::Backup::new_with_names(
    src_conn,
    options.source.as_str(),
    &mut dst_conn,
    options.target.as_str(),
  )?;

  backup.run_to_completion(
    options.rate,
    time::Duration::from_millis(250),
    None,
  )?;
  Ok(backup.progress().pagecount)
}
