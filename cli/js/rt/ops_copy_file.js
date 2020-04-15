System.register(
  "$deno$/ops/fs/copy_file.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_36, context_36) {
    "use strict";
    let dispatch_json_ts_10;
    const __moduleName = context_36 && context_36.id;
    function copyFileSync(fromPath, toPath) {
      dispatch_json_ts_10.sendSync("op_copy_file", {
        from: fromPath,
        to: toPath,
      });
    }
    exports_36("copyFileSync", copyFileSync);
    async function copyFile(fromPath, toPath) {
      await dispatch_json_ts_10.sendAsync("op_copy_file", {
        from: fromPath,
        to: toPath,
      });
    }
    exports_36("copyFile", copyFile);
    return {
      setters: [
        function (dispatch_json_ts_10_1) {
          dispatch_json_ts_10 = dispatch_json_ts_10_1;
        },
      ],
      execute: function () {},
    };
  }
);
