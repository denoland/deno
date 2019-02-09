// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { Console, libdeno, stringifyArgs, inspect, write, stdout } from "deno";
import { test, assertEqual, assert } from "./test_util.ts";

const console = new Console(libdeno.print);

// tslint:disable-next-line:no-any
function stringify(...args: any[]): string {
  return stringifyArgs(args);
}

test(function consoleTestAssertShouldNotThrowError() {
  console.assert(true);

  let hasThrown = undefined;
  try {
    console.assert(false);
    hasThrown = false;
  } catch {
    hasThrown = true;
  }
  assertEqual(hasThrown, false);
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
  // tslint:disable-next-line:max-line-length
  const nestedObjExpected = `{ num: 1, bool: true, str: "a", method: [Function: method], asyncMethod: [AsyncFunction: asyncMethod], generatorMethod: [GeneratorFunction: generatorMethod], un: undefined, nu: null, arrowFunc: [Function: arrowFunc], extendedClass: Extended { a: 1, b: 2 }, nFunc: [Function], extendedCstr: [Function: Extended], o: { num: 2, bool: false, str: "b", method: [Function: method], un: undefined, nu: null, nested: [Circular], emptyObj: {}, arr: [ 1, "s", false, null, [Circular] ], baseClass: Base { a: 1 } } }`;

  assertEqual(stringify(1), "1");
  assertEqual(stringify(1n), "1n");
  assertEqual(stringify("s"), "s");
  assertEqual(stringify(false), "false");
  // tslint:disable-next-line:no-construct
  assertEqual(stringify(new Number(1)), "[Number: 1]");
  // tslint:disable-next-line:no-construct
  assertEqual(stringify(new Boolean(true)), "[Boolean: true]");
  // tslint:disable-next-line:no-construct
  assertEqual(stringify(new String("deno")), `[String: "deno"]`);
  assertEqual(stringify(/[0-9]*/), "/[0-9]*/");
  assertEqual(
    stringify(new Date("2018-12-10T02:26:59.002Z")),
    "2018-12-10T02:26:59.002Z"
  );
  assertEqual(stringify(new Set([1, 2, 3])), "Set { 1, 2, 3 }");
  assertEqual(
    stringify(new Map([[1, "one"], [2, "two"]])),
    `Map { 1 => "one", 2 => "two" }`
  );
  assertEqual(stringify(new WeakSet()), "WeakSet { [items unknown] }");
  assertEqual(stringify(new WeakMap()), "WeakMap { [items unknown] }");
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
  assertEqual(stringify(new Uint8Array([1, 2, 3])), "Uint8Array [ 1, 2, 3 ]");
  assertEqual(stringify(Uint8Array.prototype), "TypedArray []");
  assertEqual(
    stringify({ a: { b: { c: { d: new Set([1]) } } } }),
    "{ a: { b: { c: { d: [Set] } } } }"
  );
  assertEqual(stringify(nestedObj), nestedObjExpected);
  assertEqual(stringify(JSON), "{}");
  assertEqual(
    stringify(console),
    // tslint:disable-next-line:max-line-length
    "Console { printFunc: [Function], log: [Function], debug: [Function], info: [Function], dir: [Function], warn: [Function], error: [Function], assert: [Function], count: [Function], countReset: [Function], table: [Function], time: [Function], timeLog: [Function], timeEnd: [Function], group: [Function], groupCollapsed: [Function], groupEnd: [Function], clear: [Function], indentLevel: 0, collapsedAt: null }"
  );
  // test inspect is working the same
  assertEqual(inspect(nestedObj), nestedObjExpected);
});

test(function consoleTestStringifyWithDepth() {
  // tslint:disable-next-line:no-any
  const nestedObj: any = { a: { b: { c: { d: { e: { f: 42 } } } } } };
  assertEqual(
    stringifyArgs([nestedObj], { depth: 3 }),
    "{ a: { b: { c: [Object] } } }"
  );
  assertEqual(
    stringifyArgs([nestedObj], { depth: 4 }),
    "{ a: { b: { c: { d: [Object] } } } }"
  );
  assertEqual(stringifyArgs([nestedObj], { depth: 0 }), "[Object]");
  assertEqual(
    stringifyArgs([nestedObj], { depth: null }),
    "{ a: { b: { c: { d: [Object] } } } }"
  );
  // test inspect is working the same way
  assertEqual(
    inspect(nestedObj, { depth: 4 }),
    "{ a: { b: { c: { d: [Object] } } } }"
  );
});

test(function consoleTestWithIntegerFormatSpecifier() {
  assertEqual(stringify("%i"), "%i");
  assertEqual(stringify("%i", 42.0), "42");
  assertEqual(stringify("%i", 42), "42");
  assertEqual(stringify("%i", "42"), "42");
  assertEqual(stringify("%i", "42.0"), "42");
  assertEqual(stringify("%i", 1.5), "1");
  assertEqual(stringify("%i", -0.5), "0");
  assertEqual(stringify("%i", ""), "NaN");
  assertEqual(stringify("%i", Symbol()), "NaN");
  assertEqual(stringify("%i %d", 42, 43), "42 43");
  assertEqual(stringify("%d %i", 42), "42 %i");
  assertEqual(stringify("%d", 12345678901234567890123), "1");
  assertEqual(
    stringify("%i", 12345678901234567890123n),
    "12345678901234567890123n"
  );
});

test(function consoleTestWithFloatFormatSpecifier() {
  assertEqual(stringify("%f"), "%f");
  assertEqual(stringify("%f", 42.0), "42");
  assertEqual(stringify("%f", 42), "42");
  assertEqual(stringify("%f", "42"), "42");
  assertEqual(stringify("%f", "42.0"), "42");
  assertEqual(stringify("%f", 1.5), "1.5");
  assertEqual(stringify("%f", -0.5), "-0.5");
  assertEqual(stringify("%f", Math.PI), "3.141592653589793");
  assertEqual(stringify("%f", ""), "NaN");
  assertEqual(stringify("%f", Symbol("foo")), "NaN");
  assertEqual(stringify("%f", 5n), "5");
  assertEqual(stringify("%f %f", 42, 43), "42 43");
  assertEqual(stringify("%f %f", 42), "42 %f");
});

test(function consoleTestWithStringFormatSpecifier() {
  assertEqual(stringify("%s"), "%s");
  assertEqual(stringify("%s", undefined), "undefined");
  assertEqual(stringify("%s", "foo"), "foo");
  assertEqual(stringify("%s", 42), "42");
  assertEqual(stringify("%s", "42"), "42");
  assertEqual(stringify("%s %s", 42, 43), "42 43");
  assertEqual(stringify("%s %s", 42), "42 %s");
  assertEqual(stringify("%s", Symbol("foo")), "Symbol(foo)");
});

test(function consoleTestWithObjectFormatSpecifier() {
  assertEqual(stringify("%o"), "%o");
  assertEqual(stringify("%o", 42), "42");
  assertEqual(stringify("%o", "foo"), "foo");
  assertEqual(stringify("o: %o, a: %O", {}, []), "o: {}, a: []");
  assertEqual(stringify("%o", { a: 42 }), "{ a: 42 }");
  assertEqual(
    stringify("%o", { a: { b: { c: { d: new Set([1]) } } } }),
    "{ a: { b: { c: { d: [Set] } } } }"
  );
});

test(function consoleTestWithVariousOrInvalidFormatSpecifier() {
  assertEqual(stringify("%s:%s"), "%s:%s");
  assertEqual(stringify("%i:%i"), "%i:%i");
  assertEqual(stringify("%d:%d"), "%d:%d");
  assertEqual(stringify("%%s%s", "foo"), "%sfoo");
  assertEqual(stringify("%s:%s", undefined), "undefined:%s");
  assertEqual(stringify("%s:%s", "foo", "bar"), "foo:bar");
  assertEqual(stringify("%s:%s", "foo", "bar", "baz"), "foo:bar baz");
  assertEqual(stringify("%%%s%%", "hi"), "%hi%");
  assertEqual(stringify("%d:%d", 12), "12:%d");
  assertEqual(stringify("%i:%i", 12), "12:%i");
  assertEqual(stringify("%f:%f", 12), "12:%f");
  assertEqual(stringify("o: %o, a: %o", {}), "o: {}, a: %o");
  assertEqual(stringify("abc%", 1), "abc% 1");
});

test(function consoleTestCallToStringOnLabel() {
  const methods = ["count", "countReset", "time", "timeLog", "timeEnd"];

  for (const method of methods) {
    let hasCalled = false;

    console[method]({
      toString() {
        hasCalled = true;
      }
    });

    assertEqual(hasCalled, true);
  }
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
    assert(
      stringify(e)
        .split("\n")[3]
        .includes("MyError: This is an error")
    );
  }
});

test(function consoleTestClear() {
  const stdoutWrite = stdout.write;
  const uint8 = new TextEncoder().encode("\x1b[1;1H" + "\x1b[0J");
  let buffer = new Uint8Array(0);

  stdout.write = async u8 => {
    const tmp = new Uint8Array(buffer.length + u8.length);
    tmp.set(buffer, 0);
    tmp.set(u8, buffer.length);
    buffer = tmp;

    return await write(stdout.rid, u8);
  };
  console.clear();
  stdout.write = stdoutWrite;
  assertEqual(buffer, uint8);
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
  const consoleCount = console.count;
  const consoleCountReset = console.countReset;
  const consoleTable = console.table;
  const consoleTime = console.time;
  const consoleTimeLog = console.timeLog;
  const consoleTimeEnd = console.timeEnd;
  const consoleGroup = console.group;
  const consoleGroupEnd = console.groupEnd;
  const consoleClear = console.clear;
  log("Hello world");
  dir("Hello world");
  debug("Hello world");
  info("Hello world");
  warn("Hello world");
  error("Hello world");
  consoleAssert(true);
  consoleCount("Hello world");
  consoleCountReset("Hello world");
  consoleTable({ test: "Hello world" });
  consoleTime("Hello world");
  consoleTimeLog("Hello world");
  consoleTimeEnd("Hello world");
  consoleGroup("Hello world");
  consoleGroupEnd();
  consoleClear();
});
