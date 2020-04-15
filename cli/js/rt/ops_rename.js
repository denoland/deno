System.register(
  "$deno$/ops/fs/rename.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_58, context_58) {
    "use strict";
    let dispatch_json_ts_26;
    const __moduleName = context_58 && context_58.id;
    function renameSync(oldpath, newpath) {
      dispatch_json_ts_26.sendSync("op_rename", { oldpath, newpath });
    }
    exports_58("renameSync", renameSync);
    async function rename(oldpath, newpath) {
      await dispatch_json_ts_26.sendAsync("op_rename", { oldpath, newpath });
    }
    exports_58("rename", rename);
    return {
      setters: [
        function (dispatch_json_ts_26_1) {
          dispatch_json_ts_26 = dispatch_json_ts_26_1;
        },
      ],
      execute: function () {},
    };
  }
);
