System.register(
  "$deno$/ops/worker_host.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_97, context_97) {
    "use strict";
    let dispatch_json_ts_36;
    const __moduleName = context_97 && context_97.id;
    function createWorker(specifier, hasSourceCode, sourceCode, name) {
      return dispatch_json_ts_36.sendSync("op_create_worker", {
        specifier,
        hasSourceCode,
        sourceCode,
        name,
      });
    }
    exports_97("createWorker", createWorker);
    function hostTerminateWorker(id) {
      dispatch_json_ts_36.sendSync("op_host_terminate_worker", { id });
    }
    exports_97("hostTerminateWorker", hostTerminateWorker);
    function hostPostMessage(id, data) {
      dispatch_json_ts_36.sendSync("op_host_post_message", { id }, data);
    }
    exports_97("hostPostMessage", hostPostMessage);
    function hostGetMessage(id) {
      return dispatch_json_ts_36.sendAsync("op_host_get_message", { id });
    }
    exports_97("hostGetMessage", hostGetMessage);
    return {
      setters: [
        function (dispatch_json_ts_36_1) {
          dispatch_json_ts_36 = dispatch_json_ts_36_1;
        },
      ],
      execute: function () {},
    };
  }
);
