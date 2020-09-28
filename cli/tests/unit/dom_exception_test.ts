// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, unitTest } from "./test_util.ts";

unitTest(function testDomError() {
  const de = new DOMException("foo", "bar");
  assert(de);
  assertEquals(de.message, "foo");
  assertEquals(de.name, "bar");
});
