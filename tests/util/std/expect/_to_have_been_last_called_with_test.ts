// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { fn } from "./fn.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toHaveBeenLastCalledWith()", () => {
  const mockFn = fn();

  mockFn(1, 2, 3);
  mockFn(4, 5, 6);

  expect(mockFn).toHaveBeenLastCalledWith(4, 5, 6);

  expect(mockFn).not.toHaveBeenLastCalledWith(1, 2, 3);

  assertThrows(() => {
    expect(mockFn).toHaveBeenLastCalledWith(1, 2, 3);
  }, AssertionError);

  assertThrows(() => {
    expect(mockFn).not.toHaveBeenLastCalledWith(4, 5, 6);
  }, AssertionError);
});
