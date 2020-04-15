System.register(
  "$deno$/ops/fs/make_temp.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_42, context_42) {
    "use strict";
    let dispatch_json_ts_15;
    const __moduleName = context_42 && context_42.id;
    function makeTempDirSync(options = {}) {
      return dispatch_json_ts_15.sendSync("op_make_temp_dir", options);
    }
    exports_42("makeTempDirSync", makeTempDirSync);
    function makeTempDir(options = {}) {
      return dispatch_json_ts_15.sendAsync("op_make_temp_dir", options);
    }
    exports_42("makeTempDir", makeTempDir);
    function makeTempFileSync(options = {}) {
      return dispatch_json_ts_15.sendSync("op_make_temp_file", options);
    }
    exports_42("makeTempFileSync", makeTempFileSync);
    function makeTempFile(options = {}) {
      return dispatch_json_ts_15.sendAsync("op_make_temp_file", options);
    }
    exports_42("makeTempFile", makeTempFile);
    return {
      setters: [
        function (dispatch_json_ts_15_1) {
          dispatch_json_ts_15 = dispatch_json_ts_15_1;
        },
      ],
      execute: function () {},
    };
  }
);
