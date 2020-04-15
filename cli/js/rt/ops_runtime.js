// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/runtime.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_17, context_17) {
    "use strict";
    let dispatch_json_ts_2;
    const __moduleName = context_17 && context_17.id;
    function start() {
      return dispatch_json_ts_2.sendSync("op_start");
    }
    exports_17("start", start);
    function metrics() {
      return dispatch_json_ts_2.sendSync("op_metrics");
    }
    exports_17("metrics", metrics);
    return {
      setters: [
        function (dispatch_json_ts_2_1) {
          dispatch_json_ts_2 = dispatch_json_ts_2_1;
        },
      ],
      execute: function () {},
    };
  }
);
