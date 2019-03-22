// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, test } from "./test_util.ts";

// Some of these APIs aren't exposed in the types and so we have to cast to any
// in order to "trick" TypeScript.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { Console, stringifyArgs, inspect, write, stdout } = Deno as any;

function stringify(...args: unknown[]): string {
  return stringifyArgs(args).replace(/\n$/, "");
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
  assertEquals(hasThrown, false);
});

test(function consoleTestStringifyComplexObjects() {
  assertEquals(stringify("foo"), "foo");
  assertEquals(stringify(["foo", "bar"]), `[ "foo", "bar" ]`);
  assertEquals(stringify({ foo: "bar" }), `{ foo: "bar" }`);
});

test(function consoleTestStringifyLongStrings() {
  const veryLongString = "a".repeat(200);
  // If we stringify an object containing the long string, it gets abbreviated.
  let actual = stringify({ veryLongString });
  assert(actual.includes("..."));
  assert(actual.length < 200);
  // However if we stringify the string itself, we get it exactly.
  actual = stringify(veryLongString);
  assertEquals(actual, veryLongString);
});

test(function consoleTestStringifyCircular() {
  class Base {
    a = 1;
    m1(): void {}
  }

  class Extended extends Base {
    b = 2;
    m2(): void {}
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
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
  const nestedObjExpected = `{ num, bool, str, method, asyncMethod, generatorMethod, un, nu, arrowFunc, extendedClass, nFunc, extendedCstr, o }`;

  assertEquals(stringify(1), "1");
  assertEquals(stringify(1n), "1n");
  assertEquals(stringify("s"), "s");
  assertEquals(stringify(false), "false");
  assertEquals(stringify(new Number(1)), "[Number: 1]");
  assertEquals(stringify(new Boolean(true)), "[Boolean: true]");
  assertEquals(stringify(new String("deno")), `[String: "deno"]`);
  assertEquals(stringify(/[0-9]*/), "/[0-9]*/");
  assertEquals(
    stringify(new Date("2018-12-10T02:26:59.002Z")),
    "2018-12-10T02:26:59.002Z"
  );
  assertEquals(stringify(new Set([1, 2, 3])), "Set { 1, 2, 3 }");
  assertEquals(
    stringify(new Map([[1, "one"], [2, "two"]])),
    `Map { 1 => "one", 2 => "two" }`
  );
  assertEquals(stringify(new WeakSet()), "WeakSet { [items unknown] }");
  assertEquals(stringify(new WeakMap()), "WeakMap { [items unknown] }");
  assertEquals(stringify(Symbol(1)), "Symbol(1)");
  assertEquals(stringify(null), "null");
  assertEquals(stringify(undefined), "undefined");
  assertEquals(stringify(new Extended()), "Extended { a: 1, b: 2 }");
  assertEquals(stringify(function f() {}), "[Function: f]");
  assertEquals(stringify(async function af() {}), "[AsyncFunction: af]");
  assertEquals(stringify(function* gf() {}), "[GeneratorFunction: gf]");
  assertEquals(
    stringify(async function* agf() {}),
    "[AsyncGeneratorFunction: agf]"
  );
  assertEquals(stringify(new Uint8Array([1, 2, 3])), "Uint8Array [ 1, 2, 3 ]");
  assertEquals(stringify(Uint8Array.prototype), "TypedArray []");
  assertEquals(
    stringify({ a: { b: { c: { d: new Set([1]) } } } }),
    "{ a: { b: { c: { d: [Set] } } } }"
  );
  assertEquals(stringify(nestedObj), nestedObjExpected);
  assertEquals(stringify(JSON), "{}");
  assertEquals(
    stringify(console),
    "Console { printFunc, log, debug, info, dir, warn, error, assert, count, countReset, table, time, timeLog, timeEnd, group, groupCollapsed, groupEnd, clear, indentLevel, collapsedAt }"
  );
  // test inspect is working the same
  assertEquals(inspect(nestedObj), nestedObjExpected);
});

test(function consoleTestStringifyWithDepth() {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const nestedObj: any = { a: { b: { c: { d: { e: { f: 42 } } } } } };
  assertEquals(
    stringifyArgs([nestedObj], { depth: 3 }),
    "{ a: { b: { c: [Object] } } }\n"
  );
  assertEquals(
    stringifyArgs([nestedObj], { depth: 4 }),
    "{ a: { b: { c: { d: [Object] } } } }\n"
  );
  assertEquals(stringifyArgs([nestedObj], { depth: 0 }), "[Object]\n");
  assertEquals(
    stringifyArgs([nestedObj], { depth: null }),
    "{ a: { b: { c: { d: [Object] } } } }\n"
  );
  // test inspect is working the same way
  assertEquals(
    inspect(nestedObj, { depth: 4 }),
    "{ a: { b: { c: { d: [Object] } } } }"
  );
});

test(function consoleTestWithIntegerFormatSpecifier() {
  assertEquals(stringify("%i"), "%i");
  assertEquals(stringify("%i", 42.0), "42");
  assertEquals(stringify("%i", 42), "42");
  assertEquals(stringify("%i", "42"), "42");
  assertEquals(stringify("%i", "42.0"), "42");
  assertEquals(stringify("%i", 1.5), "1");
  assertEquals(stringify("%i", -0.5), "0");
  assertEquals(stringify("%i", ""), "NaN");
  assertEquals(stringify("%i", Symbol()), "NaN");
  assertEquals(stringify("%i %d", 42, 43), "42 43");
  assertEquals(stringify("%d %i", 42), "42 %i");
  assertEquals(stringify("%d", 12345678901234567890123), "1");
  assertEquals(
    stringify("%i", 12345678901234567890123n),
    "12345678901234567890123n"
  );
});

test(function consoleTestWithFloatFormatSpecifier() {
  assertEquals(stringify("%f"), "%f");
  assertEquals(stringify("%f", 42.0), "42");
  assertEquals(stringify("%f", 42), "42");
  assertEquals(stringify("%f", "42"), "42");
  assertEquals(stringify("%f", "42.0"), "42");
  assertEquals(stringify("%f", 1.5), "1.5");
  assertEquals(stringify("%f", -0.5), "-0.5");
  assertEquals(stringify("%f", Math.PI), "3.141592653589793");
  assertEquals(stringify("%f", ""), "NaN");
  assertEquals(stringify("%f", Symbol("foo")), "NaN");
  assertEquals(stringify("%f", 5n), "5");
  assertEquals(stringify("%f %f", 42, 43), "42 43");
  assertEquals(stringify("%f %f", 42), "42 %f");
});

test(function consoleTestWithStringFormatSpecifier() {
  assertEquals(stringify("%s"), "%s");
  assertEquals(stringify("%s", undefined), "undefined");
  assertEquals(stringify("%s", "foo"), "foo");
  assertEquals(stringify("%s", 42), "42");
  assertEquals(stringify("%s", "42"), "42");
  assertEquals(stringify("%s %s", 42, 43), "42 43");
  assertEquals(stringify("%s %s", 42), "42 %s");
  assertEquals(stringify("%s", Symbol("foo")), "Symbol(foo)");
});

test(function consoleTestWithObjectFormatSpecifier() {
  assertEquals(stringify("%o"), "%o");
  assertEquals(stringify("%o", 42), "42");
  assertEquals(stringify("%o", "foo"), "foo");
  assertEquals(stringify("o: %o, a: %O", {}, []), "o: {}, a: []");
  assertEquals(stringify("%o", { a: 42 }), "{ a: 42 }");
  assertEquals(
    stringify("%o", { a: { b: { c: { d: new Set([1]) } } } }),
    "{ a: { b: { c: { d: [Set] } } } }"
  );
});

test(function consoleTestWithVariousOrInvalidFormatSpecifier() {
  assertEquals(stringify("%s:%s"), "%s:%s");
  assertEquals(stringify("%i:%i"), "%i:%i");
  assertEquals(stringify("%d:%d"), "%d:%d");
  assertEquals(stringify("%%s%s", "foo"), "%sfoo");
  assertEquals(stringify("%s:%s", undefined), "undefined:%s");
  assertEquals(stringify("%s:%s", "foo", "bar"), "foo:bar");
  assertEquals(stringify("%s:%s", "foo", "bar", "baz"), "foo:bar baz");
  assertEquals(stringify("%%%s%%", "hi"), "%hi%");
  assertEquals(stringify("%d:%d", 12), "12:%d");
  assertEquals(stringify("%i:%i", 12), "12:%i");
  assertEquals(stringify("%f:%f", 12), "12:%f");
  assertEquals(stringify("o: %o, a: %o", {}), "o: {}, a: %o");
  assertEquals(stringify("abc%", 1), "abc% 1");
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

    assertEquals(hasCalled, true);
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
  assertEquals(buffer, uint8);
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

class StringBuffer {
  chunks: string[] = [];
  add(x: string): void {
    this.chunks.push(x);
  }
  toString(): string {
    return this.chunks.join("");
  }
}

type ConsoleExamineFunc = (
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  csl: any,
  out: StringBuffer,
  err?: StringBuffer,
  both?: StringBuffer
) => void;

function mockConsole(f: ConsoleExamineFunc): void {
  const out = new StringBuffer();
  const err = new StringBuffer();
  const both = new StringBuffer();
  const csl = new Console(
    (x: string, isErr: boolean, printsNewLine: boolean) => {
      const content = x + (printsNewLine ? "\n" : "");
      const buf = isErr ? err : out;
      buf.add(content);
      both.add(content);
    }
  );
  f(csl, out, err, both);
}

// console.group test
test(function consoleGroup() {
  mockConsole((console, out) => {
    console.group("1");
    console.log("2");
    console.group("3");
    console.log("4");
    console.groupEnd();
    console.groupEnd();

    console.groupCollapsed("5");
    console.log("6");
    console.group("7");
    console.log("8");
    console.groupEnd();
    console.groupEnd();
    console.log("9");
    console.log("10");

    assertEquals(
      out.toString(),
      `1
  2
  3
    4
5678
9
10
`
    );
  });
});

// console.group with console.warn test
test(function consoleGroupWarn() {
  mockConsole((console, _out, _err, both) => {
    console.warn("1");
    console.group();
    console.warn("2");
    console.group();
    console.warn("3");
    console.groupEnd();
    console.warn("4");
    console.groupEnd();
    console.warn("5");

    console.groupCollapsed();
    console.warn("6");
    console.group();
    console.warn("7");
    console.groupEnd();
    console.warn("8");
    console.groupEnd();

    console.warn("9");
    console.warn("10");
    assertEquals(
      both.toString(),
      `1
  2
    3
  4
5
678
9
10
`
    );
  });
});

// console.table test
test(function consoleTable() {
  mockConsole((console, out) => {
    console.table({ a: "test", b: 1 });
    assertEquals(
      out.toString(),
      `┌─────────┬────────┐
│ (index) │ Values │
├─────────┼────────┤
│    a    │ "test" │
│    b    │   1    │
└─────────┴────────┘
`
    );
  });
  mockConsole((console, out) => {
    console.table({ a: { b: 10 }, b: { b: 20, c: 30 } }, ["c"]);
    assertEquals(
      out.toString(),
      `┌─────────┬────┐
│ (index) │ c  │
├─────────┼────┤
│    a    │    │
│    b    │ 30 │
└─────────┴────┘
`
    );
  });
  mockConsole((console, out) => {
    console.table([1, 2, [3, [4]], [5, 6], [[7], [8]]]);
    assertEquals(
      out.toString(),
      `┌─────────┬───────┬───────┬────────┐
│ (index) │   0   │   1   │ Values │
├─────────┼───────┼───────┼────────┤
│    0    │       │       │   1    │
│    1    │       │       │   2    │
│    2    │   3   │ [ 4 ] │        │
│    3    │   5   │   6   │        │
│    4    │ [ 7 ] │ [ 8 ] │        │
└─────────┴───────┴───────┴────────┘
`
    );
  });
  mockConsole((console, out) => {
    console.table(new Set([1, 2, 3, "test"]));
    assertEquals(
      out.toString(),
      `┌───────────────────┬────────┐
│ (iteration index) │ Values │
├───────────────────┼────────┤
│         0         │   1    │
│         1         │   2    │
│         2         │   3    │
│         3         │ "test" │
└───────────────────┴────────┘
`
    );
  });
  mockConsole((console, out) => {
    console.table(new Map([[1, "one"], [2, "two"]]));
    assertEquals(
      out.toString(),
      `┌───────────────────┬─────┬────────┐
│ (iteration index) │ Key │ Values │
├───────────────────┼─────┼────────┤
│         0         │  1  │ "one"  │
│         1         │  2  │ "two"  │
└───────────────────┴─────┴────────┘
`
    );
  });
  mockConsole((console, out) => {
    console.table({
      a: true,
      b: { c: { d: 10 }, e: [1, 2, [5, 6]] },
      f: "test",
      g: new Set([1, 2, 3, "test"]),
      h: new Map([[1, "one"]])
    });
    assertEquals(
      out.toString(),
      `┌─────────┬───────────┬───────────────────┬────────┐
│ (index) │     c     │         e         │ Values │
├─────────┼───────────┼───────────────────┼────────┤
│    a    │           │                   │  true  │
│    b    │ { d: 10 } │ [ 1, 2, [Array] ] │        │
│    f    │           │                   │ "test" │
│    g    │           │                   │        │
│    h    │           │                   │        │
└─────────┴───────────┴───────────────────┴────────┘
`
    );
  });
  mockConsole((console, out) => {
    console.table([
      1,
      "test",
      false,
      { a: 10 },
      ["test", { b: 20, c: "test" }]
    ]);
    assertEquals(
      out.toString(),
      `┌─────────┬────────┬──────────────────────┬────┬────────┐
│ (index) │   0    │          1           │ a  │ Values │
├─────────┼────────┼──────────────────────┼────┼────────┤
│    0    │        │                      │    │   1    │
│    1    │        │                      │    │ "test" │
│    2    │        │                      │    │ false  │
│    3    │        │                      │ 10 │        │
│    4    │ "test" │ { b: 20, c: "test" } │    │        │
└─────────┴────────┴──────────────────────┴────┴────────┘
`
    );
  });
  mockConsole((console, out) => {
    console.table([]);
    assertEquals(
      out.toString(),
      `┌─────────┐
│ (index) │
├─────────┤
└─────────┘
`
    );
  });
  mockConsole((console, out) => {
    console.table({});
    assertEquals(
      out.toString(),
      `┌─────────┐
│ (index) │
├─────────┤
└─────────┘
`
    );
  });
  mockConsole((console, out) => {
    console.table(new Set());
    assertEquals(
      out.toString(),
      `┌───────────────────┐
│ (iteration index) │
├───────────────────┤
└───────────────────┘
`
    );
  });
  mockConsole((console, out) => {
    console.table(new Map());
    assertEquals(
      out.toString(),
      `┌───────────────────┐
│ (iteration index) │
├───────────────────┤
└───────────────────┘
`
    );
  });
  mockConsole((console, out) => {
    console.table("test");
    assertEquals(out.toString(), "test\n");
  });
});
