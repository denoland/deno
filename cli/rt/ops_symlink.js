System.register(
  "$deno$/ops/fs/symlink.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/util.ts", "$deno$/build.ts"],
  function (exports_61, context_61) {
    "use strict";
    let dispatch_json_ts_28, util, build_ts_5;
    const __moduleName = context_61 && context_61.id;
    function symlinkSync(oldpath, newpath, type) {
      if (build_ts_5.build.os === "win" && type) {
        return util.notImplemented();
      }
      dispatch_json_ts_28.sendSync("op_symlink", { oldpath, newpath });
    }
    exports_61("symlinkSync", symlinkSync);
    async function symlink(oldpath, newpath, type) {
      if (build_ts_5.build.os === "win" && type) {
        return util.notImplemented();
      }
      await dispatch_json_ts_28.sendAsync("op_symlink", { oldpath, newpath });
    }
    exports_61("symlink", symlink);
    return {
      setters: [
        function (dispatch_json_ts_28_1) {
          dispatch_json_ts_28 = dispatch_json_ts_28_1;
        },
        function (util_5) {
          util = util_5;
        },
        function (build_ts_5_1) {
          build_ts_5 = build_ts_5_1;
        },
      ],
      execute: function () {},
    };
  }
);
