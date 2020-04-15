System.register(
  "$deno$/ops/fs/seek.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_30, context_30) {
    "use strict";
    let dispatch_json_ts_8;
    const __moduleName = context_30 && context_30.id;
    function seekSync(rid, offset, whence) {
      return dispatch_json_ts_8.sendSync("op_seek", { rid, offset, whence });
    }
    exports_30("seekSync", seekSync);
    function seek(rid, offset, whence) {
      return dispatch_json_ts_8.sendAsync("op_seek", { rid, offset, whence });
    }
    exports_30("seek", seek);
    return {
      setters: [
        function (dispatch_json_ts_8_1) {
          dispatch_json_ts_8 = dispatch_json_ts_8_1;
        },
      ],
      execute: function () {},
    };
  }
);
