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

  const nestedObjExpected = `{ num: 1, bool: true, str: "a", method: [Function: method], un: undefined, nu: null, arrowFunc: [Function: arrowFunc], extendedClass: Extended { a: 1, b: 2 }, nFunc: [Function], extendedCstr: [Function: Extended], o: { num: 2, bool: false, str: "b", method: [Function: method], un: undefined, nu: null, nested: [Circular], emptyObj: {}, arr: [ 1, "s", false, null, [Circular] ], baseClass: Base { a: 1 } } }`;

  assertEqual(stringify(1), "1");
  assertEqual(stringify("s"), "s");
  assertEqual(stringify(false), "false");
  assertEqual(stringify(Symbol(1)), "Symbol(1)");
  assertEqual(stringify(null), "null");
  assertEqual(stringify(undefined), "undefined");
  assertEqual(stringify(new Extended()), "Extended { a: 1, b: 2 }");
  assertEqual(stringify(function f() {}), "[Function: f]");
  assertEqual(stringify(nestedObj), nestedObjExpected);
  assertEqual(stringify(JSON), "{}");
  assertEqual(stringify(console), "Console { printFunc: [Function], debug: [Function: log], info: [Function: log], error: [Function: warn] }");
});
