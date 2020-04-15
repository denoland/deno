// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/core.ts", [], function (exports_5, context_5) {
  "use strict";
  const __moduleName = context_5 && context_5.id;
  return {
    setters: [],
    execute: function () {
      // This allows us to access core in API even if we
      // dispose window.Deno
      exports_5("core", globalThis.Deno.core);
    },
  };
});
