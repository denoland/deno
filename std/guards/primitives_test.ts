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

import * as primitives from "./primitives.ts";
import { assertEquals } from "../testing/asserts.ts";

const { test } = Deno;

const getUndefined = (): undefined => {
  return undefined;
};

const getBoolean = (): boolean => {
  const values = [true, false, Boolean(0), Boolean(1)];
  const index = Math.floor(Math.random() * values.length);
  return values[index];
};

const getNumber = (): number => {
  const values = [42, 3.14, Infinity, NaN, Number("42")];
  const index = Math.floor(Math.random() * values.length);
  return values[index];
};

const getString = (): string => {
  const values = ["str", String(42)];
  const index = Math.floor(Math.random() * values.length);
  return values[index];
};

// const getBigInt = () => {
//   const values = [42n, BigInt("42")]
//   const index = Math.floor(Math.random()*values.length)
//   return values[index]
// }

const getSymbol = (): symbol => {
  return Symbol("symbol");
};

test("isUndefined", (): void => {
  assertEquals(primitives.isUndefined(getUndefined()), true);
  assertEquals(primitives.isUndefined(getBoolean()), false);
  assertEquals(primitives.isUndefined(getNumber()), false);
  assertEquals(primitives.isUndefined(getString()), false);
  // assertEquals(primitives.isUndefined(getBigInt()), false)
  assertEquals(primitives.isUndefined(getSymbol()), false);
});

test("isBoolean", (): void => {
  assertEquals(primitives.isBoolean(getUndefined()), false);
  assertEquals(primitives.isBoolean(getBoolean()), true);
  assertEquals(primitives.isBoolean(getNumber()), false);
  assertEquals(primitives.isBoolean(getString()), false);
  // assertEquals(primitives.isBoolean(getBigInt()), false)
  assertEquals(primitives.isBoolean(getSymbol()), false);
});

test("isNumber", (): void => {
  assertEquals(primitives.isNumber(getUndefined()), false);
  assertEquals(primitives.isNumber(getBoolean()), false);
  assertEquals(primitives.isNumber(getNumber()), true);
  assertEquals(primitives.isNumber(getString()), false);
  // assertEquals(primitives.isNumber(getBigInt()), false)
  assertEquals(primitives.isNumber(getSymbol()), false);
});

test("isString", (): void => {
  assertEquals(primitives.isString(getUndefined()), false);
  assertEquals(primitives.isString(getBoolean()), false);
  assertEquals(primitives.isString(getNumber()), false);
  assertEquals(primitives.isString(getString()), true);
  // assertEquals(primitives.isString(getBigInt()), false)
  assertEquals(primitives.isString(getSymbol()), false);
});

// test("isBigInt", t => {
// assertEquals(primitives.isBigInt(getUndefined()), false)
// assertEquals(primitives.isBigInt(getBoolean()), false)
// assertEquals(primitives.isBigInt(getNumber()), false)
// assertEquals(primitives.isBigInt(getString()), false)
// assertEquals(primitives.isBigInt(getBigInt()), true)
// assertEquals(primitives.isBigInt(getSymbol()), false)
// })

test("isSymbol", (): void => {
  assertEquals(primitives.isSymbol(getUndefined()), false);
  assertEquals(primitives.isSymbol(getBoolean()), false);
  assertEquals(primitives.isSymbol(getNumber()), false);
  assertEquals(primitives.isSymbol(getString()), false);
  // assertEquals(primitives.isSymbol(getBigInt()), false)
  assertEquals(primitives.isSymbol(getSymbol()), true);
});
