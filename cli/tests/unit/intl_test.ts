// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test("Intl.v8BreakIterator should be undefined", () => {
  // @ts-expect-error Intl.v8BreakIterator is not a standard API
  assertEquals(Intl.v8BreakIterator, undefined);
});
