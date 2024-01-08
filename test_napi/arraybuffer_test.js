// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals, loadTestLibrary } from "./common.js";

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
