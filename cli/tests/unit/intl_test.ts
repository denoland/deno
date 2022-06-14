// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test("Intl.v8BreakIterator should be undefined", () => {
  // @ts-expect-error
  assertEquals(Intl.v8BreakIterator, undefined);
});
