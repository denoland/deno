System.register(
  "$deno$/ops/process.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/util.ts", "$deno$/build.ts"],
  function (exports_51, context_51) {
    "use strict";
    let dispatch_json_ts_21, util_ts_7, build_ts_3;
    const __moduleName = context_51 && context_51.id;
    function kill(pid, signo) {
      if (build_ts_3.build.os === "win") {
        throw new Error("Not yet implemented");
      }
      dispatch_json_ts_21.sendSync("op_kill", { pid, signo });
    }
    exports_51("kill", kill);
    function runStatus(rid) {
      return dispatch_json_ts_21.sendAsync("op_run_status", { rid });
    }
    exports_51("runStatus", runStatus);
    function run(request) {
      util_ts_7.assert(request.cmd.length > 0);
      return dispatch_json_ts_21.sendSync("op_run", request);
    }
    exports_51("run", run);
    return {
      setters: [
        function (dispatch_json_ts_21_1) {
          dispatch_json_ts_21 = dispatch_json_ts_21_1;
        },
        function (util_ts_7_1) {
          util_ts_7 = util_ts_7_1;
        },
        function (build_ts_3_1) {
          build_ts_3 = build_ts_3_1;
        },
      ],
      execute: function () {},
    };
  }
);
