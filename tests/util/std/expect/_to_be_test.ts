// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toBe()", () => {
  const obj = {};
  expect(1).toBe(1);
  expect("hello").toBe("hello");
  expect(obj).toBe(obj);

  expect(1).not.toBe(2);
  expect("hello").not.toBe("world");
  expect(obj).not.toBe({});

  assertThrows(() => {
    expect(1).toBe(2);
  }, AssertionError);
  assertThrows(() => {
    expect("hello").toBe("world");
  }, AssertionError);
  assertThrows(() => {
    expect(obj).toBe({});
  }, AssertionError);

  assertThrows(() => {
    expect(1).not.toBe(1);
  }, AssertionError);
  assertThrows(() => {
    expect("hello").not.toBe("hello");
  }, AssertionError);
  assertThrows(() => {
    expect(obj).not.toBe(obj);
  }, AssertionError);
});
