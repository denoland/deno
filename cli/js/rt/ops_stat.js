System.register(
  "$deno$/ops/fs/stat.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/file_info.ts"],
  function (exports_38, context_38) {
    "use strict";
    let dispatch_json_ts_12, file_info_ts_1;
    const __moduleName = context_38 && context_38.id;
    async function lstat(path) {
      const res = await dispatch_json_ts_12.sendAsync("op_stat", {
        path,
        lstat: true,
      });
      return new file_info_ts_1.FileInfoImpl(res);
    }
    exports_38("lstat", lstat);
    function lstatSync(path) {
      const res = dispatch_json_ts_12.sendSync("op_stat", {
        path,
        lstat: true,
      });
      return new file_info_ts_1.FileInfoImpl(res);
    }
    exports_38("lstatSync", lstatSync);
    async function stat(path) {
      const res = await dispatch_json_ts_12.sendAsync("op_stat", {
        path,
        lstat: false,
      });
      return new file_info_ts_1.FileInfoImpl(res);
    }
    exports_38("stat", stat);
    function statSync(path) {
      const res = dispatch_json_ts_12.sendSync("op_stat", {
        path,
        lstat: false,
      });
      return new file_info_ts_1.FileInfoImpl(res);
    }
    exports_38("statSync", statSync);
    return {
      setters: [
        function (dispatch_json_ts_12_1) {
          dispatch_json_ts_12 = dispatch_json_ts_12_1;
        },
        function (file_info_ts_1_1) {
          file_info_ts_1 = file_info_ts_1_1;
        },
      ],
      execute: function () {},
    };
  }
);
