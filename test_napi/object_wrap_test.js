// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

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
