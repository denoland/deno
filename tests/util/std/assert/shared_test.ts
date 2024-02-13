// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assertArrayIncludes,
  assertEquals,
  assertNotEquals,
  assertNotStrictEquals,
  assertStrictEquals,
} from "./mod.ts";

Deno.test({
  name: "assert* functions with specified type parameter",
  fn() {
    assertEquals<string>("hello", "hello");
    assertNotEquals<number>(1, 2);
    assertArrayIncludes<boolean>([true, false], [true]);
    const value = { x: 1 };
    assertStrictEquals<typeof value>(value, value);
    assertNotStrictEquals<object>(value, { x: 1 });
  },
});
