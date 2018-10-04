// Copyright 2018 the Deno authors. All rights reserved. MIT license.

import { test, assert, assertEqual } from "./test_util.ts";
import { stringifyArgs } from "./console.ts";

// tslint:disable-next-line:no-any
function stringify(...args: any[]): string {
  return stringifyArgs(args);
}

test(function consoleTestAssert() {
  console.assert(true);

  let hasThrown = false;
  try {
    console.assert(false);
  } catch {
    hasThrown = true;
  }
  assertEqual(hasThrown, true);
});

test(function consoleTestStringifyComplexObjects() {
  assertEqual(stringify("foo"), "foo");
  assertEqual(stringify(["foo", "bar"]), `[ "foo", "bar" ]`);
  assertEqual(stringify({ foo: "bar" }), `{ foo: "bar" }`);
});

test(function consoleTestStringifyCircular() {
  class Base {
    a = 1;
    m1() {}
  }

  class Extended extends Base {
    b = 2;
    m2() {}
  }

  // tslint:disable-next-line:no-any
  const nestedObj: any = {
    num: 1,
    bool: true,
    str: "a",
    method() {},
    async asyncMethod() {},
    *generatorMethod() {},
    un: undefined,
    nu: null,
    arrowFunc: () => {},
    extendedClass: new Extended(),
    nFunc: new Function(),
    extendedCstr: Extended
  };

  const circularObj = {
    num: 2,
    bool: false,
    str: "b",
    method() {},
    un: undefined,
    nu: null,
    nested: nestedObj,
    emptyObj: {},
    arr: [1, "s", false, null, nestedObj],
    baseClass: new Base()
  };

  nestedObj.o = circularObj;

  const nestedObjExpected = `{ num: 1, bool: true, str: "a", method: [Function: method], asyncMethod: [AsyncFunction: asyncMethod], generatorMethod: [GeneratorFunction: generatorMethod], un: undefined, nu: null, arrowFunc: [Function: arrowFunc], extendedClass: Extended { a: 1, b: 2 }, nFunc: [Function], extendedCstr: [Function: Extended], o: { num: 2, bool: false, str: "b", method: [Function: method], un: undefined, nu: null, nested: [Circular], emptyObj: [object], arr: [object], baseClass: [object] } }`;

  assertEqual(stringify(1), "1");
  assertEqual(stringify("s"), "s");
  assertEqual(stringify(false), "false");
  assertEqual(stringify(Symbol(1)), "Symbol(1)");
  assertEqual(stringify(null), "null");
  assertEqual(stringify(undefined), "undefined");
  assertEqual(stringify(new Extended()), "Extended { a: 1, b: 2 }");
  assertEqual(stringify(function f() {}), "[Function: f]");
  assertEqual(stringify(async function af() {}), "[AsyncFunction: af]");
  assertEqual(stringify(function* gf() {}), "[GeneratorFunction: gf]");
  assertEqual(
    stringify(async function* agf() {}),
    "[AsyncGeneratorFunction: agf]"
  );
  assertEqual(stringify(nestedObj), nestedObjExpected);
  assertEqual(stringify(JSON), "{}");
  assertEqual(
    stringify(console),
    "Console { printFunc: [Function], log: [Function], debug: [Function], info: [Function], dir: [Function], warn: [Function], error: [Function], assert: [Function] }"
  );
});

test(function consoleTestStringifyWithDepth() {
  const nestedObj: any = { a: { b: { c: { d: { e: { f: 42 } } } } } };
  assertEqual(
    stringifyArgs([nestedObj], { depth: 3 }),
    "{ a: { b: { c: [object] } } }"
  );
  assertEqual(
    stringifyArgs([nestedObj], { depth: 4 }),
    "{ a: { b: { c: { d: [object] } } } }"
  );
  assertEqual(stringifyArgs([nestedObj], { depth: 0 }), "[object]");
  assertEqual(
    stringifyArgs([nestedObj], { depth: null }),
    "{ a: { b: [object] } }"
  );
});

test(function consoleTestError() {
  class MyError extends Error {
    constructor(errStr: string) {
      super(errStr);
      this.name = "MyError";
    }
  }
  try {
    throw new MyError("This is an error");
  } catch (e) {
    assertEqual(stringify(e).split("\n")[0], "MyError: This is an error");
  }
});

// Test bound this issue
test(function consoleDetachedLog() {
  const log = console.log;
  const dir = console.dir;
  const debug = console.debug;
  const info = console.info;
  const warn = console.warn;
  const error = console.error;
  const consoleAssert = console.assert;
  log("Hello world");
  dir("Hello world");
  debug("Hello world");
  info("Hello world");
  warn("Hello world");
  error("Hello world");
  consoleAssert(true);
});
