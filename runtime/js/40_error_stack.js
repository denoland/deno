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
    const res = core.opSync("op_apply_source_map", location);
    return {
      fileName: res.fileName,
      lineNumber: res.lineNumber,
      columnNumber: res.columnNumber,
    };
  }

  window.__bootstrap.errorStack = {
    opFormatDiagnostics,
    opFormatFileName,
    opApplySourceMap,
  };
})(this);
