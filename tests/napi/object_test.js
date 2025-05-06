// Copyright 2018-2025 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertThrows,
  loadTestLibrary,
} from "./common.js";

const object = loadTestLibrary();

Deno.test("napi object", function () {
  const r = object.test_object_new(1, "hello");
  assertEquals(typeof r, "object");
  assertEquals(r[0], 1);
  assertEquals(r[1], "hello");

  const r1 = object.test_object_get(r);
  assert(r === r1);

  const r2 = object.test_object_attr_property(r);
  assert(r === r2);
  assertThrows(
    () => {
      r2.self = "2";
    },
    Error,
    "Cannot assign to read only property 'self' of object '#<Object>'",
  );

  assertThrows(
    () => {
      r2.method = () => {};
    },
    Error,
    "Cannot assign to read only property 'method' of object '#<Object>'",
  );
});
