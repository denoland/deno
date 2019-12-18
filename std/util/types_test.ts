// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { test } from "../testing/mod.ts";
import { assert } from "../testing/asserts.ts";
import{isBoolean,isNull,isNullOrUndefined,isNumber,isString,isUndefined,isError,isObject,isFunction,isRegExp,isArray,isSymbol } from "./types.ts";

test({
  name: "[types] isBoolean",
  fn() {
    assert(isBoolean(true));
    assert(isBoolean(new Boolean()));
    assert(isBoolean(new Boolean(true)));
    assert(isBoolean(false));
    assert(!isBoolean("deno"));
    assert(!isBoolean("true"));
  }
});

test({
  name: "[types] isNull",
  fn() {
    let n;
    assert(isNull(null));
    assert(!isNull(n));
    assert(!isNull(0));
    assert(!isNull({}));
  }
});

test({
  name: "[types] isNullOrUndefined",
  fn() {
    let n;
    assert(isNullOrUndefined(null));
    assert(isNullOrUndefined(n));
    assert(!isNullOrUndefined({}));
    assert(!isNullOrUndefined("undefined"));
  }
});

test({
  name: "[types] isNumber",
  fn() {
    assert(isNumber(666));
    assert(isNumber(new Number(666)));
    assert(!isNumber("999"));
    assert(!isNumber(null));
    assert(isNumber(0x0f));
  }
});

test({
  name: "[types] isString",
  fn() {
    assert(isString("deno"));
    assert(isString(new String("DIO")));
    assert(!isString(1337));
  }
});

test({
  name: "[types] isSymbol",
  fn() {
		assert(isSymbol(Symbol()))
		assert(isSymbol(Symbol("foo")))
		assert(!isSymbol("Symbol"))
	}
});

test({
  name: "[types] isUndefined",
  fn() {
    let t;
    assert(isUndefined(t));
    assert(!isUndefined("undefined"));
    assert(!isUndefined({}));
  }
});

test({
  name: "[types] isObject",
  fn() {
    const dio = { stand: "Za Warudo" };
    assert(isObject(dio));
    assert(isObject(new RegExp(/Toki Wo Tomare/)));
    assert(!isObject("Jotaro"));
  }
});

test({
  name: "[types] isError",
  fn() {
    const java = new Error();
    const nodejs = new TypeError();
    const deno = "Future";
    assert(isError(java));
    assert(isError(nodejs));
    assert(!isError(deno));
  }
});

test({
  name: "[types] isFunction",
  fn() {
    const f = function(): void {};
    assert(isFunction(f));
    assert(!isFunction({}));
    assert(!isFunction(new RegExp(/f/)));
  }
});

test({
  name: "[types] isRegExp",
  fn() {
    assert(isRegExp(new RegExp(/f/)));
    assert(isRegExp(/fuManchu/));
    assert(!isRegExp({ evil: "eye" }));
    assert(!isRegExp(null));
  }
});

test({
  name: "[types] isArray",
  fn() {
    assert(isArray([]));
    assert(!isArray({ yaNo: "array" }));
    assert(!isArray(null));
  }
});
