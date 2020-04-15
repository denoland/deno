// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/web_worker.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_108, context_108) {
    "use strict";
    let dispatch_json_ts_38;
    const __moduleName = context_108 && context_108.id;
    function postMessage(data) {
      dispatch_json_ts_38.sendSync("op_worker_post_message", {}, data);
    }
    exports_108("postMessage", postMessage);
    function close() {
      dispatch_json_ts_38.sendSync("op_worker_close");
    }
    exports_108("close", close);
    return {
      setters: [
        function (dispatch_json_ts_38_1) {
          dispatch_json_ts_38 = dispatch_json_ts_38_1;
        },
      ],
      execute: function () {},
    };
  }
);
