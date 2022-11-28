// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

function assertEquals(a, b) {
  if (a === b) return;
  throw a + " does not equal " + b;
}

const registry = new FinalizationRegistry((value) => {
  assertEquals(value, "called!");
  Deno.core.print("FinalizationRegistry called!\n");
});

(function () {
  let x = {};
  registry.register(x, "called!");
  x = null;
})();

gc();
