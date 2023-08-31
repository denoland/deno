// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

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
