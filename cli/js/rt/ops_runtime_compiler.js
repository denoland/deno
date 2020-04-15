// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/runtime_compiler.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_25, context_25) {
    "use strict";
    let dispatch_json_ts_6;
    const __moduleName = context_25 && context_25.id;
    function compile(request) {
      return dispatch_json_ts_6.sendAsync("op_compile", request);
    }
    exports_25("compile", compile);
    function transpile(request) {
      return dispatch_json_ts_6.sendAsync("op_transpile", request);
    }
    exports_25("transpile", transpile);
    return {
      setters: [
        function (dispatch_json_ts_6_1) {
          dispatch_json_ts_6 = dispatch_json_ts_6_1;
        },
      ],
      execute: function () {},
    };
  }
);
