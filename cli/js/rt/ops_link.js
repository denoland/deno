System.register(
  "$deno$/ops/fs/link.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_41, context_41) {
    "use strict";
    let dispatch_json_ts_14;
    const __moduleName = context_41 && context_41.id;
    function linkSync(oldpath, newpath) {
      dispatch_json_ts_14.sendSync("op_link", { oldpath, newpath });
    }
    exports_41("linkSync", linkSync);
    async function link(oldpath, newpath) {
      await dispatch_json_ts_14.sendAsync("op_link", { oldpath, newpath });
    }
    exports_41("link", link);
    return {
      setters: [
        function (dispatch_json_ts_14_1) {
          dispatch_json_ts_14 = dispatch_json_ts_14_1;
        },
      ],
      execute: function () {},
    };
  }
);
