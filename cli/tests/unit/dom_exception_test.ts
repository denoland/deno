// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, unitTest } from "./test_util.ts";

unitTest(function testDomError() {
  const de = new DOMException("foo", "bar");
  assert(de);
  assertEquals(de.message, "foo");
  assertEquals(de.name, "bar");
  assertEquals(de.code, 0);
});

unitTest(function testKnownDomException() {
  const de = new DOMException("foo", "SyntaxError");
  assert(de);
  assertEquals(de.message, "foo");
  assertEquals(de.name, "SyntaxError");
  assertEquals(de.code, 12);
});
