// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toStrictEqual()", () => {
  const obj = { a: 1 };
  expect(1).toStrictEqual(1);
  expect(obj).toStrictEqual(obj);

  expect(1).not.toStrictEqual(2);
  expect(obj).not.toStrictEqual({ a: 1 });

  assertThrows(() => {
    expect(1).toStrictEqual(2);
  }, AssertionError);
  assertThrows(() => {
    expect(obj).toStrictEqual({ a: 1 });
  }, AssertionError);

  assertThrows(() => {
    expect(1).not.toStrictEqual(1);
  }, AssertionError);
  assertThrows(() => {
    expect(obj).not.toStrictEqual(obj);
  }, AssertionError);
});
