System.register(
  "$deno$/ops/resources.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_28, context_28) {
    "use strict";
    let dispatch_json_ts_7;
    const __moduleName = context_28 && context_28.id;
    function resources() {
      const res = dispatch_json_ts_7.sendSync("op_resources");
      const resources = {};
      for (const resourceTuple of res) {
        resources[resourceTuple[0]] = resourceTuple[1];
      }
      return resources;
    }
    exports_28("resources", resources);
    function close(rid) {
      dispatch_json_ts_7.sendSync("op_close", { rid });
    }
    exports_28("close", close);
    return {
      setters: [
        function (dispatch_json_ts_7_1) {
          dispatch_json_ts_7 = dispatch_json_ts_7_1;
        },
      ],
      execute: function () {},
    };
  }
);
