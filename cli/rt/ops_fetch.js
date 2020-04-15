// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/fetch.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_92, context_92) {
    "use strict";
    let dispatch_json_ts_35;
    const __moduleName = context_92 && context_92.id;
    function fetch(args, body) {
      let zeroCopy = undefined;
      if (body) {
        zeroCopy = new Uint8Array(
          body.buffer,
          body.byteOffset,
          body.byteLength
        );
      }
      return dispatch_json_ts_35.sendAsync("op_fetch", args, zeroCopy);
    }
    exports_92("fetch", fetch);
    return {
      setters: [
        function (dispatch_json_ts_35_1) {
          dispatch_json_ts_35 = dispatch_json_ts_35_1;
        },
      ],
      execute: function () {},
    };
  }
);
