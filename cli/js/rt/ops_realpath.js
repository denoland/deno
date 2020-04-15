System.register(
  "$deno$/ops/fs/realpath.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_56, context_56) {
    "use strict";
    let dispatch_json_ts_24;
    const __moduleName = context_56 && context_56.id;
    function realpathSync(path) {
      return dispatch_json_ts_24.sendSync("op_realpath", { path });
    }
    exports_56("realpathSync", realpathSync);
    function realpath(path) {
      return dispatch_json_ts_24.sendAsync("op_realpath", { path });
    }
    exports_56("realpath", realpath);
    return {
      setters: [
        function (dispatch_json_ts_24_1) {
          dispatch_json_ts_24 = dispatch_json_ts_24_1;
        },
      ],
      execute: function () {},
    };
  }
);
