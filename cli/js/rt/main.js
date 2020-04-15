// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "cli/js/main.ts",
  ["$deno$/runtime_main.ts", "$deno$/runtime_worker.ts"],
  function (exports_110, context_110) {
    "use strict";
    let runtime_main_ts_1, runtime_worker_ts_1;
    const __moduleName = context_110 && context_110.id;
    return {
      setters: [
        function (runtime_main_ts_1_1) {
          runtime_main_ts_1 = runtime_main_ts_1_1;
        },
        function (runtime_worker_ts_1_1) {
          runtime_worker_ts_1 = runtime_worker_ts_1_1;
        },
      ],
      execute: function () {
        // Removes the `__proto__` for security reasons.  This intentionally makes
        // Deno non compliant with ECMA-262 Annex B.2.2.1
        //
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        delete Object.prototype.__proto__;
        Object.defineProperties(globalThis, {
          bootstrapMainRuntime: {
            value: runtime_main_ts_1.bootstrapMainRuntime,
            enumerable: false,
            writable: false,
            configurable: false,
          },
          bootstrapWorkerRuntime: {
            value: runtime_worker_ts_1.bootstrapWorkerRuntime,
            enumerable: false,
            writable: false,
            configurable: false,
          },
        });
      },
    };
  }
);
