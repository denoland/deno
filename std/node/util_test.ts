const { test } = Deno;
import { assert } from "../testing/asserts.ts";
import * as util from "./util.ts";

test({
  name: "[util] isBoolean",
  fn() {
    assert(util.isBoolean(true));
    assert(util.isBoolean(new Boolean()));
    assert(util.isBoolean(new Boolean(true)));
    assert(util.isBoolean(false));
    assert(!util.isBoolean("deno"));
    assert(!util.isBoolean("true"));
  },
});

test({
  name: "[util] isNull",
  fn() {
    let n;
    assert(util.isNull(null));
    assert(!util.isNull(n));
    assert(!util.isNull(0));
    assert(!util.isNull({}));
  },
});

test({
  name: "[util] isNullOrUndefined",
  fn() {
    let n;
    assert(util.isNullOrUndefined(null));
    assert(util.isNullOrUndefined(n));
    assert(!util.isNullOrUndefined({}));
    assert(!util.isNullOrUndefined("undefined"));
  },
});

test({
  name: "[util] isNumber",
  fn() {
    assert(util.isNumber(666));
    assert(util.isNumber(new Number(666)));
    assert(!util.isNumber("999"));
    assert(!util.isNumber(null));
  },
});

test({
  name: "[util] isString",
  fn() {
    assert(util.isString("deno"));
    assert(util.isString(new String("DIO")));
    assert(!util.isString(1337));
  },
});

test({
  name: "[util] isSymbol",
  fn() {
    assert(util.isSymbol(Symbol()));
    assert(!util.isSymbol(123));
    assert(!util.isSymbol("string"));
  },
});

test({
  name: "[util] isUndefined",
  fn() {
    let t;
    assert(util.isUndefined(t));
    assert(!util.isUndefined("undefined"));
    assert(!util.isUndefined({}));
  },
});

test({
  name: "[util] isObject",
  fn() {
    const dio = { stand: "Za Warudo" };
    assert(util.isObject(dio));
    assert(util.isObject(new RegExp(/Toki Wo Tomare/)));
    assert(!util.isObject("Jotaro"));
  },
});

test({
  name: "[util] isError",
  fn() {
    const java = new Error();
    const nodejs = new TypeError();
    const deno = "Future";
    assert(util.isError(java));
    assert(util.isError(nodejs));
    assert(!util.isError(deno));
  },
});

test({
  name: "[util] isFunction",
  fn() {
    const f = function (): void {};
    assert(util.isFunction(f));
    assert(!util.isFunction({}));
    assert(!util.isFunction(new RegExp(/f/)));
  },
});

test({
  name: "[util] isRegExp",
  fn() {
    assert(util.isRegExp(new RegExp(/f/)));
    assert(util.isRegExp(/fuManchu/));
    assert(!util.isRegExp({ evil: "eye" }));
    assert(!util.isRegExp(null));
  },
});

test({
  name: "[util] isArray",
  fn() {
    assert(util.isArray([]));
    assert(!util.isArray({ yaNo: "array" }));
    assert(!util.isArray(null));
  },
});
