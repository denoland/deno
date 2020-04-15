System.register(
  "$deno$/ops/plugins.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_49, context_49) {
    "use strict";
    let dispatch_json_ts_20;
    const __moduleName = context_49 && context_49.id;
    function openPlugin(filename) {
      return dispatch_json_ts_20.sendSync("op_open_plugin", { filename });
    }
    exports_49("openPlugin", openPlugin);
    return {
      setters: [
        function (dispatch_json_ts_20_1) {
          dispatch_json_ts_20 = dispatch_json_ts_20_1;
        },
      ],
      execute: function () {},
    };
  }
);
