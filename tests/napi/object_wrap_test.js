// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { Buffer } from "node:buffer";
import { assert, assertEquals, loadTestLibrary } from "./common.js";

const objectWrap = loadTestLibrary();

Deno.test("napi object wrap new", function () {
  const obj = new objectWrap.NapiObject(0);
  assertEquals(obj.get_value(), 0);
  obj.set_value(10);
  assertEquals(obj.get_value(), 10);
  obj.increment();
  assertEquals(obj.get_value(), 11);
  obj.increment();
  obj.set_value(10);
  assertEquals(obj.get_value(), 10);
  assertEquals(objectWrap.NapiObject.factory(), 64);
});

Deno.test("napi bind finalizer", function () {
  const obj = {};
  objectWrap.test_bind_finalizer(obj);
});

Deno.test("napi external finalizer", function () {
  let obj = objectWrap.test_external_finalizer();
  assert(obj);
  obj = null;
});

Deno.test("napi external buffer", function () {
  let buf = objectWrap.test_external_buffer();
  assertEquals(buf, new Buffer([1, 2, 3]));
  buf = null;
});

Deno.test("napi external arraybuffer", function () {
  let buf = objectWrap.test_external_arraybuffer();
  assertEquals(new Uint8Array(buf), new Uint8Array([1, 2, 3]));
  buf = null;
});

Deno.test("napi object wrap userland owned", function () {
  let obj = new objectWrap.NapiObjectOwned(1);
  assertEquals(obj.get_value(), 1);
  obj = null;
  // force finalize callback to get called
  globalThis.gc();
});
