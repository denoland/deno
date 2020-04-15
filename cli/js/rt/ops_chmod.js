System.register(
  "$deno$/ops/fs/chmod.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_23, context_23) {
    "use strict";
    let dispatch_json_ts_4;
    const __moduleName = context_23 && context_23.id;
    function chmodSync(path, mode) {
      dispatch_json_ts_4.sendSync("op_chmod", { path, mode });
    }
    exports_23("chmodSync", chmodSync);
    async function chmod(path, mode) {
      await dispatch_json_ts_4.sendAsync("op_chmod", { path, mode });
    }
    exports_23("chmod", chmod);
    return {
      setters: [
        function (dispatch_json_ts_4_1) {
          dispatch_json_ts_4 = dispatch_json_ts_4_1;
        },
      ],
      execute: function () {},
    };
  }
);
