// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { fn } from "./fn.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toHaveReturnedWith()", () => {
  const mockFn = fn((x: number) => ({ foo: x + 1 }));

  mockFn(5);
  mockFn(6);

  expect(mockFn).toHaveReturnedWith({ foo: 7 });

  expect(mockFn).not.toHaveReturnedWith({ foo: 5 });

  assertThrows(() => {
    expect(mockFn).toHaveReturnedWith({ foo: 5 });
  }, AssertionError);

  assertThrows(() => {
    expect(mockFn).not.toHaveReturnedWith({ foo: 7 });
  }, AssertionError);
});
