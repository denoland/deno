System.register(
  "$deno$/ops/errors.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_14, context_14) {
    "use strict";
    let dispatch_json_ts_1;
    const __moduleName = context_14 && context_14.id;
    function formatDiagnostics(items) {
      return dispatch_json_ts_1.sendSync("op_format_diagnostic", { items });
    }
    exports_14("formatDiagnostics", formatDiagnostics);
    function applySourceMap(location) {
      const { fileName, lineNumber, columnNumber } = location;
      const res = dispatch_json_ts_1.sendSync("op_apply_source_map", {
        fileName,
        lineNumber: lineNumber,
        columnNumber: columnNumber,
      });
      return {
        fileName: res.fileName,
        lineNumber: res.lineNumber,
        columnNumber: res.columnNumber,
      };
    }
    exports_14("applySourceMap", applySourceMap);
    return {
      setters: [
        function (dispatch_json_ts_1_1) {
          dispatch_json_ts_1 = dispatch_json_ts_1_1;
        },
      ],
      execute: function () {},
    };
  }
);
