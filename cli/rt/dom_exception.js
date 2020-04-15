// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/web/dom_exception.ts", [], function (
  exports_87,
  context_87
) {
  "use strict";
  let DOMExceptionImpl;
  const __moduleName = context_87 && context_87.id;
  return {
    setters: [],
    execute: function () {
      DOMExceptionImpl = class DOMExceptionImpl extends Error {
        constructor(message = "", name = "Error") {
          super(message);
          this.#name = name;
        }
        #name;
        get name() {
          return this.#name;
        }
      };
      exports_87("DOMExceptionImpl", DOMExceptionImpl);
    },
  };
});
