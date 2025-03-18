// Copyright 2018-2025 the Deno authors. MIT license.

import { Buffer } from "node:buffer";
import { assert, assertEquals, loadTestLibrary } from "./common.js";

const typedarray = loadTestLibrary();

Deno.test("napi typedarray uint8", function () {
  const byteArray = new Uint8Array([0, 1, 2]);
  assertEquals(byteArray.length, 3);

  const byteResult = typedarray.test_multiply(byteArray, 3);
  assert(byteResult instanceof Uint8Array);
  assertEquals(byteResult.length, 3);
  assertEquals(byteResult[0], 0);
  assertEquals(byteResult[1], 3);
  assertEquals(byteResult[2], 6);
});

Deno.test("napi typedarray float64", function () {
  const doubleArray = new Float64Array([0.0, 1.1, 2.2]);
  assertEquals(doubleArray.length, 3);

  const doubleResult = typedarray.test_multiply(doubleArray, -3);
  assert(doubleResult instanceof Float64Array);
  assertEquals(doubleResult.length, 3);
  assertEquals(doubleResult[0], -0);
  assertEquals(Math.round(10 * doubleResult[1]) / 10, -3.3);
  assertEquals(Math.round(10 * doubleResult[2]) / 10, -6.6);
});

Deno.test("napi_is_buffer", () => {
  assert(!typedarray.test_is_buffer(5));
  assert(!typedarray.test_is_buffer([]));
  assert(typedarray.test_is_buffer(new Uint8Array()));
  assert(typedarray.test_is_buffer(new Uint32Array()));
  assert(typedarray.test_is_buffer(new Buffer([])));
});

// TODO(bartlomieju): this test causes segfaults when used with jemalloc.
// Node documentation provides a hint that this function is not supported by
// other runtime like electron.
// Deno.test("napi typedarray external", function () {
//   assertEquals(
//     new Uint8Array(typedarray.test_external()),
//     new Uint8Array([0, 1, 2, 3]),
//   );
// });
