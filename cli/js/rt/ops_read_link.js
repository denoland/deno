System.register(
  "$deno$/ops/fs/read_link.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_55, context_55) {
    "use strict";
    let dispatch_json_ts_23;
    const __moduleName = context_55 && context_55.id;
    function readlinkSync(path) {
      return dispatch_json_ts_23.sendSync("op_read_link", { path });
    }
    exports_55("readlinkSync", readlinkSync);
    function readlink(path) {
      return dispatch_json_ts_23.sendAsync("op_read_link", { path });
    }
    exports_55("readlink", readlink);
    return {
      setters: [
        function (dispatch_json_ts_23_1) {
          dispatch_json_ts_23 = dispatch_json_ts_23_1;
        },
      ],
      execute: function () {},
    };
  }
);
