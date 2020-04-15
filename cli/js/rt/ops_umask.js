System.register(
  "$deno$/ops/fs/umask.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_66, context_66) {
    "use strict";
    let dispatch_json_ts_32;
    const __moduleName = context_66 && context_66.id;
    function umask(mask) {
      return dispatch_json_ts_32.sendSync("op_umask", { mask });
    }
    exports_66("umask", umask);
    return {
      setters: [
        function (dispatch_json_ts_32_1) {
          dispatch_json_ts_32 = dispatch_json_ts_32_1;
        },
      ],
      execute: function () {},
    };
  }
);
