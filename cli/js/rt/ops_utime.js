System.register(
  "$deno$/ops/fs/utime.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_67, context_67) {
    "use strict";
    let dispatch_json_ts_33;
    const __moduleName = context_67 && context_67.id;
    function toSecondsFromEpoch(v) {
      return v instanceof Date ? Math.trunc(v.valueOf() / 1000) : v;
    }
    function utimeSync(path, atime, mtime) {
      dispatch_json_ts_33.sendSync("op_utime", {
        path,
        // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
        atime: toSecondsFromEpoch(atime),
        mtime: toSecondsFromEpoch(mtime),
      });
    }
    exports_67("utimeSync", utimeSync);
    async function utime(path, atime, mtime) {
      await dispatch_json_ts_33.sendAsync("op_utime", {
        path,
        // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
        atime: toSecondsFromEpoch(atime),
        mtime: toSecondsFromEpoch(mtime),
      });
    }
    exports_67("utime", utime);
    return {
      setters: [
        function (dispatch_json_ts_33_1) {
          dispatch_json_ts_33 = dispatch_json_ts_33_1;
        },
      ],
      execute: function () {},
    };
  }
);
