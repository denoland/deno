// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! There are many types of errors in Deno:
//! - AnyError: a generic wrapper that can encapsulate any type of error.
//! - JsError: a container for the error message and stack trace for exceptions
//!   thrown in JavaScript code. We use this to pretty-print stack traces.
//! - Diagnostic: these are errors that originate in TypeScript's compiler.
//!   They're similar to JsError, in that they have line numbers. But
//!   Diagnostics are compile-time type errors, whereas JsErrors are runtime
//!   exceptions.

use crate::ast::DiagnosticBuffer;
use crate::import_map::ImportMapError;
use deno_core::error::AnyError;

fn get_import_map_error_class(_: &ImportMapError) -> &'static str {
  "URIError"
}

fn get_diagnostic_class(_: &DiagnosticBuffer) -> &'static str {
  "SyntaxError"
}

pub(crate) fn get_error_class_name(e: &AnyError) -> &'static str {
  deno_runtime::errors::get_error_class_name(e)
    .or_else(|| {
      e.downcast_ref::<ImportMapError>()
        .map(get_import_map_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<DiagnosticBuffer>()
        .map(get_diagnostic_class)
    })
    .unwrap_or_else(|| {
      panic!("Error '{}' contains boxed error of unknown type", e);
    })
}
