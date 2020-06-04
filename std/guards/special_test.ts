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

import * as special from "./special.ts";
import { assertEquals } from "../testing/asserts.ts";

const { test } = Deno;

test("isNull", (): void => {
  assertEquals(special.isNull(null), true);
  assertEquals(special.isNull(undefined), false);
  assertEquals(special.isNull(0), false);
  assertEquals(special.isNull(false), false);
});

test("isFunction", (): void => {
  assertEquals(
    special.isFunction(() => {}),
    true
  );
  assertEquals(
    special.isFunction(function () {}),
    true
  );
  assertEquals(special.isFunction(class C {}), true);
  assertEquals(special.isFunction(parseInt), true);
  assertEquals(special.isFunction(null), false);
  assertEquals(special.isFunction(undefined), false);
  assertEquals(special.isFunction("str"), false);
  assertEquals(special.isFunction(42), false);
  assertEquals(special.isFunction({}), false);
  assertEquals(special.isFunction([]), false);
  assertEquals(special.isFunction(Symbol("symbol")), false);
});

test("isObject", (): void => {
  assertEquals(special.isObject({}), true);
  assertEquals(special.isObject(new (class C {})()), true);
  assertEquals(special.isObject(new Map()), true);
  assertEquals(special.isObject(new Set()), true);
  assertEquals(special.isObject(new WeakMap()), true);
  assertEquals(special.isObject(new WeakSet()), true);
  assertEquals(special.isObject(new Date()), true);
  assertEquals(
    special.isObject(() => {}),
    false
  );
  assertEquals(
    special.isObject(function () {}),
    false
  );
  assertEquals(special.isObject(class C {}), false);
  assertEquals(special.isObject(parseInt), false);
  assertEquals(special.isObject(null), false);
  assertEquals(special.isObject(undefined), false);
  assertEquals(special.isObject("str"), false);
  assertEquals(special.isObject(42), false);
  assertEquals(special.isObject([]), false);
  assertEquals(special.isObject(Symbol("symbol")), false);
});

test("isArray", (): void => {
  assertEquals(special.isArray([]), true);
  assertEquals(special.isArray(class C {}), false);
  assertEquals(special.isArray(new Map()), false);
  assertEquals(special.isArray(new Set()), false);
  assertEquals(special.isArray(new WeakMap()), false);
  assertEquals(special.isArray(new WeakSet()), false);
  assertEquals(special.isArray(new Date()), false);
  assertEquals(
    special.isArray(() => {}),
    false
  );
  assertEquals(
    special.isArray(function () {}),
    false
  );
  assertEquals(special.isArray(class C {}), false);
  assertEquals(special.isArray(parseInt), false);
  assertEquals(special.isArray(null), false);
  assertEquals(special.isArray(undefined), false);
  assertEquals(special.isArray("str"), false);
  assertEquals(special.isArray(42), false);
  assertEquals(special.isArray(Symbol("symbol")), false);
});

test("isMap", (): void => {
  assertEquals(special.isMap({}), false);
  assertEquals(special.isMap(new (class C {})()), false);
  assertEquals(special.isMap(new Map()), true);
  assertEquals(special.isMap(new Set()), false);
  assertEquals(special.isMap(new WeakMap()), false);
  assertEquals(special.isMap(new WeakSet()), false);
  assertEquals(special.isMap(new Date()), false);
  assertEquals(
    special.isMap(() => {}),
    false
  );
  assertEquals(
    special.isMap(function () {}),
    false
  );
  assertEquals(special.isMap(class C {}), false);
  assertEquals(special.isMap(parseInt), false);
  assertEquals(special.isMap(null), false);
  assertEquals(special.isMap(undefined), false);
  assertEquals(special.isMap("str"), false);
  assertEquals(special.isMap(42), false);
  assertEquals(special.isMap([]), false);
  assertEquals(special.isMap(Symbol("symbol")), false);
});

test("isSet", (): void => {
  assertEquals(special.isSet({}), false);
  assertEquals(special.isSet(new (class C {})()), false);
  assertEquals(special.isSet(new Map()), false);
  assertEquals(special.isSet(new Set()), true);
  assertEquals(special.isSet(new WeakMap()), false);
  assertEquals(special.isSet(new WeakSet()), false);
  assertEquals(special.isSet(new Date()), false);
  assertEquals(
    special.isSet(() => {}),
    false
  );
  assertEquals(
    special.isSet(function () {}),
    false
  );
  assertEquals(special.isSet(class C {}), false);
  assertEquals(special.isSet(parseInt), false);
  assertEquals(special.isSet(null), false);
  assertEquals(special.isSet(undefined), false);
  assertEquals(special.isSet("str"), false);
  assertEquals(special.isSet(42), false);
  assertEquals(special.isSet([]), false);
  assertEquals(special.isSet(Symbol("symbol")), false);
});

test("isWeakMap", (): void => {
  assertEquals(special.isWeakMap({}), false);
  assertEquals(special.isWeakMap(new (class C {})()), false);
  assertEquals(special.isWeakMap(new Map()), false);
  assertEquals(special.isWeakMap(new Set()), false);
  assertEquals(special.isWeakMap(new WeakMap()), true);
  assertEquals(special.isWeakMap(new WeakSet()), false);
  assertEquals(special.isWeakMap(new Date()), false);
  assertEquals(
    special.isWeakMap(() => {}),
    false
  );
  assertEquals(
    special.isWeakMap(function () {}),
    false
  );
  assertEquals(special.isWeakMap(class C {}), false);
  assertEquals(special.isWeakMap(parseInt), false);
  assertEquals(special.isWeakMap(null), false);
  assertEquals(special.isWeakMap(undefined), false);
  assertEquals(special.isWeakMap("str"), false);
  assertEquals(special.isWeakMap(42), false);
  assertEquals(special.isWeakMap([]), false);
  assertEquals(special.isWeakMap(Symbol("symbol")), false);
});

test("isWeakSet", (): void => {
  assertEquals(special.isWeakSet({}), false);
  assertEquals(special.isWeakSet(new (class C {})()), false);
  assertEquals(special.isWeakSet(new Map()), false);
  assertEquals(special.isWeakSet(new Set()), false);
  assertEquals(special.isWeakSet(new WeakMap()), false);
  assertEquals(special.isWeakSet(new WeakSet()), true);
  assertEquals(special.isWeakSet(new Date()), false);
  assertEquals(
    special.isWeakSet(() => {}),
    false
  );
  assertEquals(
    special.isWeakSet(function () {}),
    false
  );
  assertEquals(special.isWeakSet(class C {}), false);
  assertEquals(special.isWeakSet(parseInt), false);
  assertEquals(special.isWeakSet(null), false);
  assertEquals(special.isWeakSet(undefined), false);
  assertEquals(special.isWeakSet("str"), false);
  assertEquals(special.isWeakSet(42), false);
  assertEquals(special.isWeakSet([]), false);
  assertEquals(special.isWeakSet(Symbol("symbol")), false);
});

test("isDate", (): void => {
  assertEquals(special.isDate({}), false);
  assertEquals(special.isDate(new (class C {})()), false);
  assertEquals(special.isDate(new Map()), false);
  assertEquals(special.isDate(new Set()), false);
  assertEquals(special.isDate(new WeakMap()), false);
  assertEquals(special.isDate(new WeakSet()), false);
  assertEquals(special.isDate(new Date()), true);
  assertEquals(
    special.isDate(() => {}),
    false
  );
  assertEquals(
    special.isDate(function () {}),
    false
  );
  assertEquals(special.isDate(class C {}), false);
  assertEquals(special.isDate(parseInt), false);
  assertEquals(special.isDate(null), false);
  assertEquals(special.isDate(undefined), false);
  assertEquals(special.isDate("str"), false);
  assertEquals(special.isDate(42), false);
  assertEquals(special.isDate([]), false);
  assertEquals(special.isDate(Symbol("symbol")), false);
});
