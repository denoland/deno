// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

//! There are many types of errors in Deno:
//! - AnyError: a generic wrapper that can encapsulate any type of error.
//! - JsError: a container for the error message and stack trace for exceptions
//!   thrown in JavaScript code. We use this to pretty-print stack traces.
//! - Diagnostic: these are errors that originate in TypeScript's compiler.
//!   They're similar to JsError, in that they have line numbers. But
//!   Diagnostics are compile-time type errors, whereas JsErrors are runtime
//!   exceptions.

use crate::ast::Diagnostic;
use crate::import_map::ImportMapError;
use crate::module_graph::GraphError;
use crate::specifier_handler::HandlerError;
use deno_core::error::custom_error;
use deno_core::error::AnyError;

fn get_import_map_error_class(_: &ImportMapError) -> &'static str {
  "URIError"
}

fn get_diagnostic_class(_: &Diagnostic) -> &'static str {
  "SyntaxError"
}

fn get_graph_error_class(_: &GraphError) -> &'static str {
  "TypeError"
}

fn get_handler_error_class(error: &HandlerError) -> &'static str {
  match error {
    HandlerError::FetchErrorWithLocation(error, ..) => {
      get_error_class_name(error)
    }
  }
}

pub(crate) fn get_error_class_name(e: &AnyError) -> &'static str {
  deno_runtime::errors::get_error_class_name(e)
    .or_else(|| {
      e.downcast_ref::<ImportMapError>()
        .map(get_import_map_error_class)
    })
    .or_else(|| e.downcast_ref::<Diagnostic>().map(get_diagnostic_class))
    .or_else(|| e.downcast_ref::<GraphError>().map(get_graph_error_class))
    .or_else(|| {
      e.downcast_ref::<HandlerError>()
        .map(get_handler_error_class)
    })
    .unwrap_or_else(|| {
      panic!(
        "Error '{}' contains boxed error of unknown type:{}",
        e,
        e.chain()
          .map(|e| format!("\n  {:?}", e))
          .collect::<String>()
      );
    })
}

pub(crate) fn derive_custom_error(error: &AnyError) -> AnyError {
  custom_error(get_error_class_name(error), error.to_string())
}
