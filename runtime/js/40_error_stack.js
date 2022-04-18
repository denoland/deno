// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;

  function opFormatDiagnostics(diagnostics) {
    return core.opSync("op_format_diagnostic", diagnostics);
  }

  function opFormatFileName(location) {
    return core.opSync("op_format_file_name", location);
  }

  function opApplySourceMap(location) {
    return core.applySourceMap(location);
  }

  window.__bootstrap.errorStack = {
    opFormatDiagnostics,
    opFormatFileName,
    opApplySourceMap,
  };
})(this);
