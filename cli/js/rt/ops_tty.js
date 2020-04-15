System.register("$deno$/ops/tty.ts", ["$deno$/ops/dispatch_json.ts"], function (
  exports_65,
  context_65
) {
  "use strict";
  let dispatch_json_ts_31;
  const __moduleName = context_65 && context_65.id;
  function isatty(rid) {
    return dispatch_json_ts_31.sendSync("op_isatty", { rid });
  }
  exports_65("isatty", isatty);
  function setRaw(rid, mode) {
    dispatch_json_ts_31.sendSync("op_set_raw", {
      rid,
      mode,
    });
  }
  exports_65("setRaw", setRaw);
  return {
    setters: [
      function (dispatch_json_ts_31_1) {
        dispatch_json_ts_31 = dispatch_json_ts_31_1;
      },
    ],
    execute: function () {},
  };
});
