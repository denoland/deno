// Inspired by Elixir Guards:
// https://hexdocs.pm/elixir/guards.html
//
// Based on the latest ECMAScript standard (last updated Jun 4, 2020):
// See https://developer.mozilla.org/en-US/docs/Web/JavaScript/Data_structures
//
// Originally implemented by Slavomir Vojacek:
// https://github.com/hqoss/guards
//
// Copyright 2020, Slavomir Vojacek. All rights reserved. MIT license.

import * as convenience from "./convenience.ts";
import { assertEquals } from "../testing/asserts.ts";

const { test } = Deno;

test("isNonEmptyArray", (): void => {
  assertEquals(convenience.isNonEmptyArray([1, 2]), true);
  assertEquals(convenience.isNonEmptyArray([1]), true);
  assertEquals(convenience.isNonEmptyArray([]), false);
});

test("isValidNumber", (): void => {
  assertEquals(convenience.isValidNumber(0), true);
  assertEquals(convenience.isValidNumber(42), true);
  assertEquals(convenience.isValidNumber(-42), true);
  assertEquals(convenience.isValidNumber(3.14), true);
  assertEquals(convenience.isValidNumber(-3.14), true);
  assertEquals(convenience.isValidNumber(Infinity), true);
  assertEquals(convenience.isValidNumber(-Infinity), true);
  assertEquals(convenience.isValidNumber(Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isValidNumber(-Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isValidNumber(NaN), false);
});

test("isInteger", (): void => {
  assertEquals(convenience.isInteger(0), true);
  assertEquals(convenience.isInteger(42), true);
  assertEquals(convenience.isInteger(-42), true);
  assertEquals(convenience.isInteger(3.14), false);
  assertEquals(convenience.isInteger(-3.14), false);
  assertEquals(convenience.isInteger(Infinity), false);
  assertEquals(convenience.isInteger(-Infinity), false);
  assertEquals(convenience.isInteger(Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isInteger(-Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isInteger(NaN), false);
});

test("isPositiveInteger", (): void => {
  assertEquals(convenience.isPositiveInteger(0), false);
  assertEquals(convenience.isPositiveInteger(42), true);
  assertEquals(convenience.isPositiveInteger(-42), false);
  assertEquals(convenience.isPositiveInteger(3.14), false);
  assertEquals(convenience.isPositiveInteger(-3.14), false);
  assertEquals(convenience.isPositiveInteger(Infinity), false);
  assertEquals(convenience.isPositiveInteger(-Infinity), false);
  assertEquals(convenience.isPositiveInteger(Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isPositiveInteger(-Number.MAX_SAFE_INTEGER), false);
  assertEquals(convenience.isPositiveInteger(NaN), false);
});

test("isNonNegativeInteger", (): void => {
  assertEquals(convenience.isNonNegativeInteger(0), true);
  assertEquals(convenience.isNonNegativeInteger(42), true);
  assertEquals(convenience.isNonNegativeInteger(-42), false);
  assertEquals(convenience.isNonNegativeInteger(3.14), false);
  assertEquals(convenience.isNonNegativeInteger(-3.14), false);
  assertEquals(convenience.isNonNegativeInteger(Infinity), false);
  assertEquals(convenience.isNonNegativeInteger(-Infinity), false);
  assertEquals(convenience.isNonNegativeInteger(Number.MAX_SAFE_INTEGER), true);
  assertEquals(
    convenience.isNonNegativeInteger(-Number.MAX_SAFE_INTEGER),
    false
  );
  assertEquals(convenience.isNonNegativeInteger(NaN), false);
});

test("isNegativeInteger", (): void => {
  assertEquals(convenience.isNegativeInteger(0), false);
  assertEquals(convenience.isNegativeInteger(42), false);
  assertEquals(convenience.isNegativeInteger(-42), true);
  assertEquals(convenience.isNegativeInteger(3.14), false);
  assertEquals(convenience.isNegativeInteger(-3.14), false);
  assertEquals(convenience.isNegativeInteger(Infinity), false);
  assertEquals(convenience.isNegativeInteger(-Infinity), false);
  assertEquals(convenience.isNegativeInteger(Number.MAX_SAFE_INTEGER), false);
  assertEquals(convenience.isNegativeInteger(-Number.MAX_SAFE_INTEGER), true);
  assertEquals(convenience.isNegativeInteger(NaN), false);
});
