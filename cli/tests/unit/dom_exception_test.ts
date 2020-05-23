// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals, assert } from "./test_util.ts";

unitTest(function testDomError() {
  const de = new DOMException("foo", "bar");
  assert(de);
  assertEquals(de.message, "foo");
  assertEquals(de.name, "bar");
});
