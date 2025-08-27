// Copyright 2018-2025 the Deno authors. MIT license.

import { assert, assertEquals, loadTestLibrary } from "./common.js";
import { Buffer } from "node:buffer";

const typedarray = loadTestLibrary();

Deno.test("napi arraybuffer detach", function () {
  const buf = new ArrayBuffer(5);
  assertEquals(buf.byteLength, 5);
  typedarray.test_detached(buf);
  assertEquals(buf.byteLength, 0);
});

Deno.test("napi arraybuffer is detached", function () {
  const buf = new ArrayBuffer(5);
  assertEquals(buf.byteLength, 5);
  assert(!typedarray.is_detached(buf));
  typedarray.test_detached(buf);
  assert(typedarray.is_detached(buf));

  [2, {}, "foo", null, undefined, new Uint8Array(10)].forEach((value) => {
    assert(!typedarray.is_detached(value));
  });
});

Deno.test("napi buffer finalizer may be null", () => {
  const buf = typedarray.test_static_external_buffer();
  assertEquals(buf, Buffer.from([1, 2, 3]));
});
