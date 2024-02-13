// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { fn } from "./fn.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toHaveBeenCalledTimes()", () => {
  const mockFn = fn();
  mockFn();
  expect(mockFn).toHaveBeenCalledTimes(1);

  expect(mockFn).not.toHaveBeenCalledTimes(2);

  assertThrows(() => {
    expect(mockFn).toHaveBeenCalledTimes(2);
  }, AssertionError);

  assertThrows(() => {
    expect(mockFn).not.toHaveBeenCalledTimes(1);
  }, AssertionError);
});
