System.register("$deno$/internals.ts", [], function (exports_15, context_15) {
  "use strict";
  let internalObject;
  const __moduleName = context_15 && context_15.id;
  // Register a field to internalObject for test access,
  // through Deno[Deno.symbols.internal][name].
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  function exposeForTest(name, value) {
    Object.defineProperty(internalObject, name, {
      value,
      enumerable: false,
    });
  }
  exports_15("exposeForTest", exposeForTest);
  return {
    setters: [],
    execute: function () {
      // Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
      exports_15("internalSymbol", Symbol("Deno.internal"));
      // The object where all the internal fields for testing will be living.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      exports_15("internalObject", (internalObject = {}));
    },
  };
});
