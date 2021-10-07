// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

//! There are many types of errors in Deno:
//! - AnyError: a generic wrapper that can encapsulate any type of error.
//! - JsError: a container for the error message and stack trace for exceptions
//!   thrown in JavaScript code. We use this to pretty-print stack traces.
//! - Diagnostic: these are errors that originate in TypeScript's compiler.
//!   They're similar to JsError, in that they have line numbers. But
//!   Diagnostics are compile-time type errors, whereas JsErrors are runtime
//!   exceptions.

use crate::emit::GraphError;

use deno_ast::Diagnostic;
use deno_core::error::AnyError;
use deno_graph::ModuleGraphError;
use deno_graph::ResolutionError;
use import_map::ImportMapError;

fn get_import_map_error_class(_: &ImportMapError) -> &'static str {
  "URIError"
}

fn get_diagnostic_class(_: &Diagnostic) -> &'static str {
  "SyntaxError"
}

fn get_graph_error_class(err: &GraphError) -> &'static str {
  get_module_graph_error_class(&err.0)
}

pub(crate) fn get_module_graph_error_class(
  err: &ModuleGraphError,
) -> &'static str {
  match err {
    ModuleGraphError::LoadingErr(_, err) => get_error_class_name(err.as_ref()),
    ModuleGraphError::InvalidSource(_, _) => "SyntaxError",
    ModuleGraphError::ParseErr(_, diagnostic) => {
      get_diagnostic_class(diagnostic)
    }
    ModuleGraphError::ResolutionError(err) => get_resolution_error_class(err),
    ModuleGraphError::UnsupportedMediaType(_, _) => "TypeError",
    ModuleGraphError::Missing(_) => "NotFound",
  }
}

fn get_resolution_error_class(err: &ResolutionError) -> &'static str {
  match err {
    ResolutionError::ResolverError(err, _, _) => {
      get_error_class_name(err.as_ref())
    }
    _ => "TypeError",
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
      e.downcast_ref::<ModuleGraphError>()
        .map(get_module_graph_error_class)
    })
    .or_else(|| {
      e.downcast_ref::<ResolutionError>()
        .map(get_resolution_error_class)
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
