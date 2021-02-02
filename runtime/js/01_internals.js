"use strict";
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const internalSymbol = Symbol("Deno.internal");

  // The object where all the internal fields for testing will be living.
  const internalObject = {};

  // Register a field to internalObject for test access,
  // through Deno[Deno.internal][name].
  function exposeForTest(name, value) {
    Object.defineProperty(internalObject, name, {
      value,
      enumerable: false,
    });
  }

  window.__bootstrap.internals = {
    internalSymbol,
    internalObject,
    exposeForTest,
  };
})(this);
