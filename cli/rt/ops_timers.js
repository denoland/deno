System.register(
  "$deno$/ops/timers.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_18, context_18) {
    "use strict";
    let dispatch_json_ts_3;
    const __moduleName = context_18 && context_18.id;
    function stopGlobalTimer() {
      dispatch_json_ts_3.sendSync("op_global_timer_stop");
    }
    exports_18("stopGlobalTimer", stopGlobalTimer);
    async function startGlobalTimer(timeout) {
      await dispatch_json_ts_3.sendAsync("op_global_timer", { timeout });
    }
    exports_18("startGlobalTimer", startGlobalTimer);
    function now() {
      return dispatch_json_ts_3.sendSync("op_now");
    }
    exports_18("now", now);
    return {
      setters: [
        function (dispatch_json_ts_3_1) {
          dispatch_json_ts_3 = dispatch_json_ts_3_1;
        },
      ],
      execute: function () {},
    };
  }
);
