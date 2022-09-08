// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi_get_undefined", () => {
  assertEquals(lib.testUndefined(), undefined);
});

Deno.test("napi_get_null", () => {
  assertEquals(lib.testNull(), null);
});

Deno.test("napi_array_length", function () {
  const e = [0, "Hello", {}];
  const r = lib.testArrLen(e);
  assertEquals(r, e.length);
});

Deno.test("napi_test_int32", function () {
  assertEquals(lib.testInt32(), 69);
});

Deno.test("napi_test_int64", function () {
  assertEquals(lib.testInt64(), 9223372036854776000);
});

Deno.test("napi_test_string", function () {
  assertEquals(lib.testString("deno"), "Hello, deno!");
});

Deno.test("napi_test_bool", function () {
  assertEquals(lib.testBool(), true);
});

Deno.test("napi_test_create_obj", function () {
  const r = lib.testCreateObj();
  assertEquals(r, { test: 1 });
});

Deno.test("napi_test_get_field", function () {
  const v = new Date().toUTCString();
  const r = lib.testGetField({ a: v }, "a");
  assertEquals(r, v);
});

Deno.test("napi_object_wrap", function () {
  const r = new lib.ObjectWrap(0);
  assertEquals(r.value, 0);
  r.setValue(1);
  assertEquals(r.value, 1);
});

Deno.test("napi_tsfn", async function () {
  let n = 0;
  let resolve;
  const promise = new Promise((r) => {
    resolve = r;
  });
  lib.callThreadsafeFunction(() => {
    if (n === 99) {
      resolve();
    }
    n++;
  });
  await promise;
});

Deno.test("napi_read_file_async", async function () {
  const url = new URL("./common.js", import.meta.url);
  const buf = await lib.readFileAsync(url.pathname);
  assert(buf instanceof Uint8Array);
  const actual = await Deno.readFile(url.pathname);
  assertEquals(buf, actual);
});
