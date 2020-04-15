System.register(
  "$deno$/web/performance.ts",
  ["$deno$/ops/timers.ts"],
  function (exports_99, context_99) {
    "use strict";
    let timers_ts_3, Performance;
    const __moduleName = context_99 && context_99.id;
    return {
      setters: [
        function (timers_ts_3_1) {
          timers_ts_3 = timers_ts_3_1;
        },
      ],
      execute: function () {
        Performance = class Performance {
          now() {
            const res = timers_ts_3.now();
            return res.seconds * 1e3 + res.subsecNanos / 1e6;
          }
        };
        exports_99("Performance", Performance);
      },
    };
  }
);
