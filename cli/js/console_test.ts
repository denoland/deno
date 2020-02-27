// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, test } from "./test_util.ts";

// Some of these APIs aren't exposed in the types and so we have to cast to any
// in order to "trick" TypeScript.
const {
  inspect,
  writeSync,
  stdout
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
} = Deno as any;

const customInspect = Deno.symbols.customInspect;
const {
  Console,
  stringifyArgs
  // @ts-ignore TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.symbols.internal];

function stringify(...args: unknown[]): string {
  return stringifyArgs(args).replace(/\n$/, "");
}

// test cases from web-platform-tests
// via https://github.com/web-platform-tests/wpt/blob/master/console/console-is-a-namespace.any.js
test(function consoleShouldBeANamespace(): void {
  const prototype1 = Object.getPrototypeOf(console);
  const prototype2 = Object.getPrototypeOf(prototype1);

  assert.equals(Object.getOwnPropertyNames(prototype1).length, 0);
  assert.equals(prototype2, Object.prototype);
});

test(function consoleHasRightInstance(): void {
  assert(console instanceof Console);
  assert.equals({} instanceof Console, false);
});

test(function consoleTestAssertShouldNotThrowError(): void {
  console.assert(true);

  let hasThrown = undefined;
  try {
    console.assert(false);
    hasThrown = false;
  } catch {
    hasThrown = true;
  }
  assert.equals(hasThrown, false);
});

test(function consoleTestStringifyComplexObjects(): void {
  assert.equals(stringify("foo"), "foo");
  assert.equals(stringify(["foo", "bar"]), `[ "foo", "bar" ]`);
  assert.equals(stringify({ foo: "bar" }), `{ foo: "bar" }`);
});

test(function consoleTestStringifyLongStrings(): void {
  const veryLongString = "a".repeat(200);
  // If we stringify an object containing the long string, it gets abbreviated.
  let actual = stringify({ veryLongString });
  assert(actual.includes("..."));
  assert(actual.length < 200);
  // However if we stringify the string itself, we get it exactly.
  actual = stringify(veryLongString);
  assert.equals(actual, veryLongString);
});

/* eslint-disable @typescript-eslint/explicit-function-return-type */
test(function consoleTestStringifyCircular(): void {
  class Base {
    a = 1;
    m1() {}
  }

  class Extended extends Base {
    b = 2;
    m2() {}
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

  assert.equals(stringify(1), "1");
  assert.equals(stringify(-0), "-0");
  assert.equals(stringify(1n), "1n");
  assert.equals(stringify("s"), "s");
  assert.equals(stringify(false), "false");
  assert.equals(stringify(new Number(1)), "[Number: 1]");
  assert.equals(stringify(new Boolean(true)), "[Boolean: true]");
  assert.equals(stringify(new String("deno")), `[String: "deno"]`);
  assert.equals(stringify(/[0-9]*/), "/[0-9]*/");
  assert.equals(
    stringify(new Date("2018-12-10T02:26:59.002Z")),
    "2018-12-10T02:26:59.002Z"
  );
  assert.equals(stringify(new Set([1, 2, 3])), "Set { 1, 2, 3 }");
  assert.equals(
    stringify(
      new Map([
        [1, "one"],
        [2, "two"]
      ])
    ),
    `Map { 1 => "one", 2 => "two" }`
  );
  assert.equals(stringify(new WeakSet()), "WeakSet { [items unknown] }");
  assert.equals(stringify(new WeakMap()), "WeakMap { [items unknown] }");
  assert.equals(stringify(Symbol(1)), "Symbol(1)");
  assert.equals(stringify(null), "null");
  assert.equals(stringify(undefined), "undefined");
  assert.equals(stringify(new Extended()), "Extended { a: 1, b: 2 }");
  assert.equals(
    stringify(function f(): void {}),
    "[Function: f]"
  );
  assert.equals(
    stringify(async function af(): Promise<void> {}),
    "[AsyncFunction: af]"
  );
  assert.equals(
    stringify(function* gf() {}),
    "[GeneratorFunction: gf]"
  );
  assert.equals(
    stringify(async function* agf() {}),
    "[AsyncGeneratorFunction: agf]"
  );
  assert.equals(stringify(new Uint8Array([1, 2, 3])), "Uint8Array [ 1, 2, 3 ]");
  assert.equals(stringify(Uint8Array.prototype), "TypedArray []");
  assert.equals(
    stringify({ a: { b: { c: { d: new Set([1]) } } } }),
    "{ a: { b: { c: { d: [Set] } } } }"
  );
  assert.equals(stringify(nestedObj), nestedObjExpected);
  assert.equals(stringify(JSON), "{}");
  assert.equals(
    stringify(console),
    "{ printFunc, log, debug, info, dir, dirxml, warn, error, assert, count, countReset, table, time, timeLog, timeEnd, group, groupCollapsed, groupEnd, clear, trace, indentLevel }"
  );
  // test inspect is working the same
  assert.equals(inspect(nestedObj), nestedObjExpected);
});
/* eslint-enable @typescript-eslint/explicit-function-return-type */

test(function consoleTestStringifyWithDepth(): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const nestedObj: any = { a: { b: { c: { d: { e: { f: 42 } } } } } };
  assert.equals(
    stringifyArgs([nestedObj], { depth: 3 }),
    "{ a: { b: { c: [Object] } } }"
  );
  assert.equals(
    stringifyArgs([nestedObj], { depth: 4 }),
    "{ a: { b: { c: { d: [Object] } } } }"
  );
  assert.equals(stringifyArgs([nestedObj], { depth: 0 }), "[Object]");
  assert.equals(
    stringifyArgs([nestedObj], { depth: null }),
    "{ a: { b: { c: { d: [Object] } } } }"
  );
  // test inspect is working the same way
  assert.equals(
    inspect(nestedObj, { depth: 4 }),
    "{ a: { b: { c: { d: [Object] } } } }"
  );
});

test(function consoleTestWithCustomInspector(): void {
  class A {
    [customInspect](): string {
      return "b";
    }
  }

  assert.equals(stringify(new A()), "b");
});

test(function consoleTestWithCustomInspectorError(): void {
  class A {
    [customInspect](): string {
      throw new Error("BOOM");
      return "b";
    }
  }

  assert.equals(stringify(new A()), "A {}");

  class B {
    constructor(public field: { a: string }) {}
    [customInspect](): string {
      return this.field.a;
    }
  }

  assert.equals(stringify(new B({ a: "a" })), "a");
  assert.equals(stringify(B.prototype), "{}");
});

test(function consoleTestWithIntegerFormatSpecifier(): void {
  assert.equals(stringify("%i"), "%i");
  assert.equals(stringify("%i", 42.0), "42");
  assert.equals(stringify("%i", 42), "42");
  assert.equals(stringify("%i", "42"), "42");
  assert.equals(stringify("%i", "42.0"), "42");
  assert.equals(stringify("%i", 1.5), "1");
  assert.equals(stringify("%i", -0.5), "0");
  assert.equals(stringify("%i", ""), "NaN");
  assert.equals(stringify("%i", Symbol()), "NaN");
  assert.equals(stringify("%i %d", 42, 43), "42 43");
  assert.equals(stringify("%d %i", 42), "42 %i");
  assert.equals(stringify("%d", 12345678901234567890123), "1");
  assert.equals(
    stringify("%i", 12345678901234567890123n),
    "12345678901234567890123n"
  );
});

test(function consoleTestWithFloatFormatSpecifier(): void {
  assert.equals(stringify("%f"), "%f");
  assert.equals(stringify("%f", 42.0), "42");
  assert.equals(stringify("%f", 42), "42");
  assert.equals(stringify("%f", "42"), "42");
  assert.equals(stringify("%f", "42.0"), "42");
  assert.equals(stringify("%f", 1.5), "1.5");
  assert.equals(stringify("%f", -0.5), "-0.5");
  assert.equals(stringify("%f", Math.PI), "3.141592653589793");
  assert.equals(stringify("%f", ""), "NaN");
  assert.equals(stringify("%f", Symbol("foo")), "NaN");
  assert.equals(stringify("%f", 5n), "5");
  assert.equals(stringify("%f %f", 42, 43), "42 43");
  assert.equals(stringify("%f %f", 42), "42 %f");
});

test(function consoleTestWithStringFormatSpecifier(): void {
  assert.equals(stringify("%s"), "%s");
  assert.equals(stringify("%s", undefined), "undefined");
  assert.equals(stringify("%s", "foo"), "foo");
  assert.equals(stringify("%s", 42), "42");
  assert.equals(stringify("%s", "42"), "42");
  assert.equals(stringify("%s %s", 42, 43), "42 43");
  assert.equals(stringify("%s %s", 42), "42 %s");
  assert.equals(stringify("%s", Symbol("foo")), "Symbol(foo)");
});

test(function consoleTestWithObjectFormatSpecifier(): void {
  assert.equals(stringify("%o"), "%o");
  assert.equals(stringify("%o", 42), "42");
  assert.equals(stringify("%o", "foo"), "foo");
  assert.equals(stringify("o: %o, a: %O", {}, []), "o: {}, a: []");
  assert.equals(stringify("%o", { a: 42 }), "{ a: 42 }");
  assert.equals(
    stringify("%o", { a: { b: { c: { d: new Set([1]) } } } }),
    "{ a: { b: { c: { d: [Set] } } } }"
  );
});

test(function consoleTestWithVariousOrInvalidFormatSpecifier(): void {
  assert.equals(stringify("%s:%s"), "%s:%s");
  assert.equals(stringify("%i:%i"), "%i:%i");
  assert.equals(stringify("%d:%d"), "%d:%d");
  assert.equals(stringify("%%s%s", "foo"), "%sfoo");
  assert.equals(stringify("%s:%s", undefined), "undefined:%s");
  assert.equals(stringify("%s:%s", "foo", "bar"), "foo:bar");
  assert.equals(stringify("%s:%s", "foo", "bar", "baz"), "foo:bar baz");
  assert.equals(stringify("%%%s%%", "hi"), "%hi%");
  assert.equals(stringify("%d:%d", 12), "12:%d");
  assert.equals(stringify("%i:%i", 12), "12:%i");
  assert.equals(stringify("%f:%f", 12), "12:%f");
  assert.equals(stringify("o: %o, a: %o", {}), "o: {}, a: %o");
  assert.equals(stringify("abc%", 1), "abc% 1");
});

test(function consoleTestCallToStringOnLabel(): void {
  const methods = ["count", "countReset", "time", "timeLog", "timeEnd"];

  for (const method of methods) {
    let hasCalled = false;

    // @ts-ignore
    console[method]({
      toString(): void {
        hasCalled = true;
      }
    });

    assert.equals(hasCalled, true);
  }
});

test(function consoleTestError(): void {
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
        .split("\n")[0] // error has been caught
        .includes("MyError: This is an error")
    );
  }
});

test(function consoleTestClear(): void {
  const stdoutWriteSync = stdout.writeSync;
  const uint8 = new TextEncoder().encode("\x1b[1;1H" + "\x1b[0J");
  let buffer = new Uint8Array(0);

  stdout.writeSync = (u8: Uint8Array): Promise<number> => {
    const tmp = new Uint8Array(buffer.length + u8.length);
    tmp.set(buffer, 0);
    tmp.set(u8, buffer.length);
    buffer = tmp;

    return writeSync(stdout.rid, u8);
  };
  console.clear();
  stdout.writeSync = stdoutWriteSync;
  assert.equals(buffer, uint8);
});

// Test bound this issue
test(function consoleDetachedLog(): void {
  const log = console.log;
  const dir = console.dir;
  const dirxml = console.dirxml;
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
  dirxml("Hello world");
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
    (x: string, isErr: boolean, printsNewLine: boolean): void => {
      const content = x + (printsNewLine ? "\n" : "");
      const buf = isErr ? err : out;
      buf.add(content);
      both.add(content);
    }
  );
  f(csl, out, err, both);
}

// console.group test
test(function consoleGroup(): void {
  mockConsole((console, out): void => {
    console.group("1");
    console.log("2");
    console.group("3");
    console.log("4");
    console.groupEnd();
    console.groupEnd();
    console.log("5");
    console.log("6");

    assert.equals(
      out.toString(),
      `1
  2
  3
    4
5
6
`
    );
  });
});

// console.group with console.warn test
test(function consoleGroupWarn(): void {
  mockConsole((console, _out, _err, both): void => {
    assert(both);
    console.warn("1");
    console.group();
    console.warn("2");
    console.group();
    console.warn("3");
    console.groupEnd();
    console.warn("4");
    console.groupEnd();
    console.warn("5");

    console.warn("6");
    console.warn("7");
    assert.equals(
      both.toString(),
      `1
  2
    3
  4
5
6
7
`
    );
  });
});

// console.table test
test(function consoleTable(): void {
  mockConsole((console, out): void => {
    console.table({ a: "test", b: 1 });
    assert.equals(
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
  mockConsole((console, out): void => {
    console.table({ a: { b: 10 }, b: { b: 20, c: 30 } }, ["c"]);
    assert.equals(
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
  mockConsole((console, out): void => {
    console.table([1, 2, [3, [4]], [5, 6], [[7], [8]]]);
    assert.equals(
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
  mockConsole((console, out): void => {
    console.table(new Set([1, 2, 3, "test"]));
    assert.equals(
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
  mockConsole((console, out): void => {
    console.table(
      new Map([
        [1, "one"],
        [2, "two"]
      ])
    );
    assert.equals(
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
  mockConsole((console, out): void => {
    console.table({
      a: true,
      b: { c: { d: 10 }, e: [1, 2, [5, 6]] },
      f: "test",
      g: new Set([1, 2, 3, "test"]),
      h: new Map([[1, "one"]])
    });
    assert.equals(
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
  mockConsole((console, out): void => {
    console.table([
      1,
      "test",
      false,
      { a: 10 },
      ["test", { b: 20, c: "test" }]
    ]);
    assert.equals(
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
  mockConsole((console, out): void => {
    console.table([]);
    assert.equals(
      out.toString(),
      `┌─────────┐
│ (index) │
├─────────┤
└─────────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table({});
    assert.equals(
      out.toString(),
      `┌─────────┐
│ (index) │
├─────────┤
└─────────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table(new Set());
    assert.equals(
      out.toString(),
      `┌───────────────────┐
│ (iteration index) │
├───────────────────┤
└───────────────────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table(new Map());
    assert.equals(
      out.toString(),
      `┌───────────────────┐
│ (iteration index) │
├───────────────────┤
└───────────────────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table("test");
    assert.equals(out.toString(), "test\n");
  });
});

// console.log(Error) test
test(function consoleLogShouldNotThrowError(): void {
  let result = 0;
  try {
    console.log(new Error("foo"));
    result = 1;
  } catch (e) {
    result = 2;
  }
  assert.equals(result, 1);

  // output errors to the console should not include "Uncaught"
  mockConsole((console, out): void => {
    console.log(new Error("foo"));
    assert.equals(out.toString().includes("Uncaught"), false);
  });
});

// console.dir test
test(function consoleDir(): void {
  mockConsole((console, out): void => {
    console.dir("DIR");
    assert.equals(out.toString(), "DIR\n");
  });
  mockConsole((console, out): void => {
    console.dir("DIR", { indentLevel: 2 });
    assert.equals(out.toString(), "  DIR\n");
  });
});

// console.dir test
test(function consoleDirXml(): void {
  mockConsole((console, out): void => {
    console.dirxml("DIRXML");
    assert.equals(out.toString(), "DIRXML\n");
  });
  mockConsole((console, out): void => {
    console.dirxml("DIRXML", { indentLevel: 2 });
    assert.equals(out.toString(), "  DIRXML\n");
  });
});

// console.trace test
test(function consoleTrace(): void {
  mockConsole((console, _out, err): void => {
    console.trace("%s", "custom message");
    assert(err);
    assert(err.toString().includes("Trace: custom message"));
  });
});
