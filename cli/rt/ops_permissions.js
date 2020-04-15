System.register(
  "$deno$/ops/permissions.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_47, context_47) {
    "use strict";
    let dispatch_json_ts_19;
    const __moduleName = context_47 && context_47.id;
    function query(desc) {
      return dispatch_json_ts_19.sendSync("op_query_permission", desc).state;
    }
    exports_47("query", query);
    function revoke(desc) {
      return dispatch_json_ts_19.sendSync("op_revoke_permission", desc).state;
    }
    exports_47("revoke", revoke);
    function request(desc) {
      return dispatch_json_ts_19.sendSync("op_request_permission", desc).state;
    }
    exports_47("request", request);
    return {
      setters: [
        function (dispatch_json_ts_19_1) {
          dispatch_json_ts_19 = dispatch_json_ts_19_1;
        },
      ],
      execute: function () {},
    };
  }
);
