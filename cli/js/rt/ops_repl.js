// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/repl.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_103, context_103) {
    "use strict";
    let dispatch_json_ts_37;
    const __moduleName = context_103 && context_103.id;
    function startRepl(historyFile) {
      return dispatch_json_ts_37.sendSync("op_repl_start", { historyFile });
    }
    exports_103("startRepl", startRepl);
    function readline(rid, prompt) {
      return dispatch_json_ts_37.sendAsync("op_repl_readline", { rid, prompt });
    }
    exports_103("readline", readline);
    return {
      setters: [
        function (dispatch_json_ts_37_1) {
          dispatch_json_ts_37 = dispatch_json_ts_37_1;
        },
      ],
      execute: function () {},
    };
  }
);
