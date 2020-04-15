System.register(
  "$deno$/ops/fs/truncate.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_64, context_64) {
    "use strict";
    let dispatch_json_ts_30;
    const __moduleName = context_64 && context_64.id;
    function coerceLen(len) {
      if (!len) {
        return 0;
      }
      if (len < 0) {
        return 0;
      }
      return len;
    }
    function truncateSync(path, len) {
      dispatch_json_ts_30.sendSync("op_truncate", {
        path,
        len: coerceLen(len),
      });
    }
    exports_64("truncateSync", truncateSync);
    async function truncate(path, len) {
      await dispatch_json_ts_30.sendAsync("op_truncate", {
        path,
        len: coerceLen(len),
      });
    }
    exports_64("truncate", truncate);
    return {
      setters: [
        function (dispatch_json_ts_30_1) {
          dispatch_json_ts_30 = dispatch_json_ts_30_1;
        },
      ],
      execute: function () {},
    };
  }
);
