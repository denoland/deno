System.register(
  "$deno$/ops/fs/open.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_31, context_31) {
    "use strict";
    let dispatch_json_ts_9;
    const __moduleName = context_31 && context_31.id;
    function openSync(path, openMode, options) {
      const mode = options?.mode;
      return dispatch_json_ts_9.sendSync("op_open", {
        path,
        options,
        openMode,
        mode,
      });
    }
    exports_31("openSync", openSync);
    function open(path, openMode, options) {
      const mode = options?.mode;
      return dispatch_json_ts_9.sendAsync("op_open", {
        path,
        options,
        openMode,
        mode,
      });
    }
    exports_31("open", open);
    return {
      setters: [
        function (dispatch_json_ts_9_1) {
          dispatch_json_ts_9 = dispatch_json_ts_9_1;
        },
      ],
      execute: function () {},
    };
  }
);
