// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;

  function opFormatDiagnostics(diagnostics) {
    return core.jsonOpSync("op_format_diagnostic", diagnostics);
  }

  function opApplySourceMap(location) {
    const res = core.jsonOpSync("op_apply_source_map", location);
    return {
      fileName: res.fileName,
      lineNumber: res.lineNumber,
      columnNumber: res.columnNumber,
    };
  }

  window.__bootstrap.errorStack = {
    opApplySourceMap,
    opFormatDiagnostics,
  };
})(this);
