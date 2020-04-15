System.register(
  "$deno$/ops/fs/chown.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_24, context_24) {
    "use strict";
    let dispatch_json_ts_5;
    const __moduleName = context_24 && context_24.id;
    function chownSync(path, uid, gid) {
      dispatch_json_ts_5.sendSync("op_chown", { path, uid, gid });
    }
    exports_24("chownSync", chownSync);
    async function chown(path, uid, gid) {
      await dispatch_json_ts_5.sendAsync("op_chown", { path, uid, gid });
    }
    exports_24("chown", chown);
    return {
      setters: [
        function (dispatch_json_ts_5_1) {
          dispatch_json_ts_5 = dispatch_json_ts_5_1;
        },
      ],
      execute: function () {},
    };
  }
);
