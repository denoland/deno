// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// NOTE to all: use **cached** prepared statements when interfacing with SQLite.

mod indexeddb;
mod webstorage;

use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::Extension;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone)]
struct OriginStorageDir(PathBuf);

pub fn init(origin_storage_dir: Option<PathBuf>) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/webstorage",
      "01_webstorage.js",
      "02_indexeddb.js",
    ))
    .ops(vec![
      webstorage::op_webstorage_length::decl(),
      webstorage::op_webstorage_key::decl(),
      webstorage::op_webstorage_set::decl(),
      webstorage::op_webstorage_get::decl(),
      webstorage::op_webstorage_remove::decl(),
      webstorage::op_webstorage_clear::decl(),
      webstorage::op_webstorage_iterate_keys::decl(),
    ])
    .state(move |state| {
      if let Some(origin_storage_dir) = &origin_storage_dir {
        state.put(OriginStorageDir(origin_storage_dir.clone()));
      }
      Ok(())
    })
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_webstorage.d.ts")
}

#[derive(Debug)]
pub struct DomExceptionNotSupportedError {
  pub msg: String,
}

impl DomExceptionNotSupportedError {
  pub fn new(msg: &str) -> Self {
    DomExceptionNotSupportedError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DomExceptionNotSupportedError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DomExceptionNotSupportedError {}

pub fn get_not_supported_error_class_name(
  e: &AnyError,
) -> Option<&'static str> {
  e.downcast_ref::<DomExceptionNotSupportedError>()
    .map(|_| "DOMExceptionNotSupportedError")
}

#[derive(Debug)]
pub struct DomExceptionVersionError {
  pub msg: String,
}

impl DomExceptionVersionError {
  pub fn new(msg: &str) -> Self {
    DomExceptionVersionError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DomExceptionVersionError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DomExceptionVersionError {}

pub fn get_version_error_class_name(e: &AnyError) -> Option<&'static str> {
  e.downcast_ref::<DomExceptionVersionError>()
    .map(|_| "DOMExceptionVersionError")
}

#[derive(Debug)]
pub struct DomExceptionConstraintError {
  pub msg: String,
}

impl DomExceptionConstraintError {
  pub fn new(msg: &str) -> Self {
    DomExceptionConstraintError {
      msg: msg.to_string(),
    }
  }
}

impl fmt::Display for DomExceptionConstraintError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.pad(&self.msg)
  }
}

impl std::error::Error for DomExceptionConstraintError {}

pub fn get_constraint_error_class_name(e: &AnyError) -> Option<&'static str> {
  e.downcast_ref::<DomExceptionConstraintError>()
    .map(|_| "DOMExceptionConstraintError")
}
