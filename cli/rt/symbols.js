System.register(
  "$deno$/symbols.ts",
  ["$deno$/internals.ts", "$deno$/web/console.ts"],
  function (exports_70, context_70) {
    "use strict";
    let internals_ts_4, console_ts_2;
    const __moduleName = context_70 && context_70.id;
    return {
      setters: [
        function (internals_ts_4_1) {
          internals_ts_4 = internals_ts_4_1;
        },
        function (console_ts_2_1) {
          console_ts_2 = console_ts_2_1;
        },
      ],
      execute: function () {
        exports_70("symbols", {
          internal: internals_ts_4.internalSymbol,
          customInspect: console_ts_2.customInspect,
        });
      },
    };
  }
);
