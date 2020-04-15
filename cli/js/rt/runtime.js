System.register(
  "$deno$/runtime.ts",
  [
    "$deno$/core.ts",
    "$deno$/ops/dispatch_minimal.ts",
    "$deno$/ops/dispatch_json.ts",
    "$deno$/util.ts",
    "$deno$/build.ts",
    "$deno$/version.ts",
    "$deno$/error_stack.ts",
    "$deno$/ops/runtime.ts",
    "$deno$/web/timers.ts",
  ],
  function (exports_21, context_21) {
    "use strict";
    let core_ts_3,
      dispatchMinimal,
      dispatchJson,
      util,
      build_ts_1,
      version_ts_1,
      error_stack_ts_1,
      runtime_ts_1,
      timers_ts_2,
      OPS_CACHE;
    const __moduleName = context_21 && context_21.id;
    function getAsyncHandler(opName) {
      switch (opName) {
        case "op_write":
        case "op_read":
          return dispatchMinimal.asyncMsgFromRust;
        default:
          return dispatchJson.asyncMsgFromRust;
      }
    }
    // TODO(bartlomieju): temporary solution, must be fixed when moving
    // dispatches to separate crates
    function initOps() {
      exports_21("OPS_CACHE", (OPS_CACHE = core_ts_3.core.ops()));
      for (const [name, opId] of Object.entries(OPS_CACHE)) {
        core_ts_3.core.setAsyncHandler(opId, getAsyncHandler(name));
      }
      core_ts_3.core.setMacrotaskCallback(timers_ts_2.handleTimerMacrotask);
    }
    exports_21("initOps", initOps);
    function start(source) {
      initOps();
      // First we send an empty `Start` message to let the privileged side know we
      // are ready. The response should be a `StartRes` message containing the CLI
      // args and other info.
      const s = runtime_ts_1.start();
      version_ts_1.setVersions(s.denoVersion, s.v8Version, s.tsVersion);
      build_ts_1.setBuildInfo(s.os, s.arch);
      util.setLogDebug(s.debugFlag, source);
      error_stack_ts_1.setPrepareStackTrace(Error);
      return s;
    }
    exports_21("start", start);
    return {
      setters: [
        function (core_ts_3_1) {
          core_ts_3 = core_ts_3_1;
        },
        function (dispatchMinimal_1) {
          dispatchMinimal = dispatchMinimal_1;
        },
        function (dispatchJson_1) {
          dispatchJson = dispatchJson_1;
        },
        function (util_2) {
          util = util_2;
        },
        function (build_ts_1_1) {
          build_ts_1 = build_ts_1_1;
        },
        function (version_ts_1_1) {
          version_ts_1 = version_ts_1_1;
        },
        function (error_stack_ts_1_1) {
          error_stack_ts_1 = error_stack_ts_1_1;
        },
        function (runtime_ts_1_1) {
          runtime_ts_1 = runtime_ts_1_1;
        },
        function (timers_ts_2_1) {
          timers_ts_2 = timers_ts_2_1;
        },
      ],
      execute: function () {},
    };
  }
);
