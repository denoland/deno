// Copyright 2018-2025 the Deno authors. MIT license.

import { assertEquals, assertThrows, loadTestLibrary } from "./common.js";

const callback = loadTestLibrary();

Deno.test("napi callback run with args", function () {
  const result = callback.test_callback_run((a, b) => a + b, [1, 2]);
  assertEquals(result, 3);
});

Deno.test("napi callback run with args (no return)", function () {
  const result = callback.test_callback_run(() => {}, []);
  assertEquals(result, undefined);
});

Deno.test("napi callback run with args (extra arguments)", function () {
  const result = callback.test_callback_run((a, b) => a + b, [
    "Hello,",
    " Deno!",
    1,
    2,
    3,
  ]);
  assertEquals(result, "Hello, Deno!");
});

Deno.test("napi callback run with args & recv", function () {
  const result = callback.test_callback_run_with_recv(
    function () {
      assertEquals(this, 69);
      return this;
    },
    [],
    69,
  );
  assertEquals(result, 69);
});

Deno.test("napi callback handles errors correctly", function () {
  const e = new Error("hi!");
  assertThrows(() => {
    callback.test_callback_throws(() => {
      throw e;
    });
  }, e);
});
