// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { unitTest, assert, assertEquals } from "./test_util.ts";

unitTest(function canCreateAssertionError() {
  const as = new Deno.errors.AssertionError();
  assertEquals(as.message, "");
  assertEquals(as.actual, undefined);
  assertEquals(as.expected, undefined);
});

unitTest(function canPassActualExpected() {
  const as = new Deno.errors.AssertionError("test", {
    actual: "foo",
    expected: "bar",
  });
  assertEquals(as.message, "test");
  assertEquals(as.actual, "foo");
  assertEquals(as.expected, "bar");
});

unitTest(function assertionInstanceOf() {
  const as = new Deno.errors.AssertionError();
  assert(as instanceof Deno.errors.AssertionError);
  assert(as instanceof Error);
});
