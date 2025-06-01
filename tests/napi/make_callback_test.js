// Copyright 2018-2025 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const mc = loadTestLibrary();

Deno.test("napi makeCallback1", function () {
  const resource = {};

  let callCount = 0;
  function cb() {
    callCount++;
    assertEquals(arguments.length, 0);
    assertEquals(this, globalThis);
    return 42;
  }
  assertEquals(mc.makeCallback(resource, globalThis, cb), 42);
  assertEquals(callCount, 1);
});

Deno.test("napi makeCallback2", function () {
  const resource = {};

  let callCount = 0;
  function cb(x) {
    callCount++;
    assertEquals(arguments.length, 1);
    assertEquals(this, globalThis);
    assertEquals(x, 1337);
    return 42;
  }
  assertEquals(mc.makeCallback(resource, globalThis, cb, 1337), 42);
  assertEquals(callCount, 1);
});

Deno.test("napi makeCallback3", function () {
  const resource = {};

  let callCount = 0;

  function multiArgFunc(arg1, arg2, arg3) {
    callCount++;
    assertEquals(arg1, 1);
    assertEquals(arg2, 2);
    assertEquals(arg3, 3);
    return 42;
  }
  assertEquals(
    mc.makeCallback(resource, globalThis, multiArgFunc, 1, 2, 3),
    42,
  );
  assertEquals(callCount, 1);
});
