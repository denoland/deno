System.register(
  "$deno$/ops/fs/read_dir.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/file_info.ts"],
  function (exports_53, context_53) {
    "use strict";
    let dispatch_json_ts_22, file_info_ts_2;
    const __moduleName = context_53 && context_53.id;
    function res(response) {
      return response.entries.map((statRes) => {
        return new file_info_ts_2.FileInfoImpl(statRes);
      });
    }
    function readdirSync(path) {
      return res(dispatch_json_ts_22.sendSync("op_read_dir", { path }));
    }
    exports_53("readdirSync", readdirSync);
    async function readdir(path) {
      return res(await dispatch_json_ts_22.sendAsync("op_read_dir", { path }));
    }
    exports_53("readdir", readdir);
    return {
      setters: [
        function (dispatch_json_ts_22_1) {
          dispatch_json_ts_22 = dispatch_json_ts_22_1;
        },
        function (file_info_ts_2_1) {
          file_info_ts_2 = file_info_ts_2_1;
        },
      ],
      execute: function () {},
    };
  }
);
