System.register(
  "$deno$/ops/fs/dir.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_37, context_37) {
    "use strict";
    let dispatch_json_ts_11;
    const __moduleName = context_37 && context_37.id;
    function cwd() {
      return dispatch_json_ts_11.sendSync("op_cwd");
    }
    exports_37("cwd", cwd);
    function chdir(directory) {
      dispatch_json_ts_11.sendSync("op_chdir", { directory });
    }
    exports_37("chdir", chdir);
    return {
      setters: [
        function (dispatch_json_ts_11_1) {
          dispatch_json_ts_11 = dispatch_json_ts_11_1;
        },
      ],
      execute: function () {},
    };
  }
);
