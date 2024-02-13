// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { expect } from "./expect.ts";
import { AssertionError, assertThrows } from "../assert/mod.ts";

Deno.test("expect().toThrow()", () => {
  expect(() => {
    throw new Error("hello world");
  }).toThrow();

  expect(() => {}).not.toThrow();

  assertThrows(() => {
    expect(() => {}).toThrow();
  }, AssertionError);

  assertThrows(() => {
    expect(() => {
      throw new Error("hello world");
    }).not.toThrow();
  }, AssertionError);
});
