// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertRejects, loadTestLibrary } from "./common.js";

const promise = loadTestLibrary();

Deno.test("napi new promise and resolve", async () => {
  const p = promise.test_promise_new();
  promise.test_promise_resolve(69);

  assertEquals(await p, 69);
});

Deno.test("napi new promise and reject", () => {
  const p = promise.test_promise_new();

  assertRejects(async () => {
    promise.test_promise_reject(new TypeError("pikaboo"));
    await p;
  }, TypeError);
});

Deno.test("napi new promise and reject", async () => {
  const p = promise.test_promise_new();
  const is = promise.test_promise_is(p);
  assertEquals(typeof is, "boolean");
  assertEquals(is, true);

  assertEquals(promise.test_promise_is(undefined), false);
  assertEquals(promise.test_promise_is({}), false);
  promise.test_promise_resolve(69);

  assertEquals(await p, 69);
});
