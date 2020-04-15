System.register(
  "$deno$/ops/signal.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_59, context_59) {
    "use strict";
    let dispatch_json_ts_27;
    const __moduleName = context_59 && context_59.id;
    function bindSignal(signo) {
      return dispatch_json_ts_27.sendSync("op_signal_bind", { signo });
    }
    exports_59("bindSignal", bindSignal);
    function pollSignal(rid) {
      return dispatch_json_ts_27.sendAsync("op_signal_poll", { rid });
    }
    exports_59("pollSignal", pollSignal);
    function unbindSignal(rid) {
      dispatch_json_ts_27.sendSync("op_signal_unbind", { rid });
    }
    exports_59("unbindSignal", unbindSignal);
    return {
      setters: [
        function (dispatch_json_ts_27_1) {
          dispatch_json_ts_27 = dispatch_json_ts_27_1;
        },
      ],
      execute: function () {},
    };
  }
);
