System.register(
  "$deno$/ops/fs/remove.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_57, context_57) {
    "use strict";
    let dispatch_json_ts_25;
    const __moduleName = context_57 && context_57.id;
    function removeSync(path, options = {}) {
      dispatch_json_ts_25.sendSync("op_remove", {
        path,
        recursive: !!options.recursive,
      });
    }
    exports_57("removeSync", removeSync);
    async function remove(path, options = {}) {
      await dispatch_json_ts_25.sendAsync("op_remove", {
        path,
        recursive: !!options.recursive,
      });
    }
    exports_57("remove", remove);
    return {
      setters: [
        function (dispatch_json_ts_25_1) {
          dispatch_json_ts_25 = dispatch_json_ts_25_1;
        },
      ],
      execute: function () {},
    };
  }
);
