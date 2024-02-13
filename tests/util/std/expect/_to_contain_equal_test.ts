// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toContainEqual()", () => {
  const arr = [{ foo: 42 }, { bar: 43 }, { baz: 44 }];
  expect(arr).toContainEqual({ bar: 43 });

  expect(arr).not.toContainEqual({ bar: 42 });

  assertThrows(() => {
    expect(arr).toContainEqual({ bar: 42 });
  }, AssertionError);

  assertThrows(() => {
    expect(arr).not.toContainEqual({ bar: 43 });
  }, AssertionError);
});
