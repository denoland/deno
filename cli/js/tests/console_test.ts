//@ts-nocheck
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows, unitTest } from "./test_util.ts";

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
unitTest(function consoleShouldBeANamespace(): void {
  const prototype1 = Object.getPrototypeOf(console);
  const prototype2 = Object.getPrototypeOf(prototype1);

  assertEquals(Object.getOwnPropertyNames(prototype1).length, 0);
  assertEquals(prototype2, Object.prototype);
});

unitTest(function consoleHasRightInstance(): void {
  assert(console instanceof Console);
  assertEquals({} instanceof Console, false);
});

unitTest(function consoleTestAssertShouldNotThrowError(): void {
  mockConsole(console => {
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
});

unitTest(function consoleTestStringifyComplexObjects(): void {
  assertEquals(stringify("foo"), "foo");
  assertEquals(stringify(["foo", "bar"]), `[ "foo", "bar" ]`);
  assertEquals(stringify({ foo: "bar" }), `{ foo: "bar" }`);
});

unitTest(function consoleTestStringifyLongStrings(): void {
  const veryLongString = "a".repeat(200);
  // If we stringify an object containing the long string, it gets abbreviated.
  let actual = stringify({ veryLongString });
  assert(actual.includes("..."));
  assert(actual.length < 200);
  // However if we stringify the string itself, we get it exactly.
  actual = stringify(veryLongString);
  assertEquals(actual, veryLongString);
});

/* eslint-disable @typescript-eslint/explicit-function-return-type */
unitTest(function consoleTestStringifyCircular(): void {
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

  assertEquals(stringify(1), "1");
  assertEquals(stringify(-0), "-0");
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
    stringify(
      new Map([
        [1, "one"],
        [2, "two"]
      ])
    ),
    `Map { 1 => "one", 2 => "two" }`
  );
  assertEquals(stringify(new WeakSet()), "WeakSet { [items unknown] }");
  assertEquals(stringify(new WeakMap()), "WeakMap { [items unknown] }");
  assertEquals(stringify(Symbol(1)), "Symbol(1)");
  assertEquals(stringify(null), "null");
  assertEquals(stringify(undefined), "undefined");
  assertEquals(stringify(new Extended()), "Extended { a: 1, b: 2 }");
  assertEquals(
    stringify(function f(): void {}),
    "[Function: f]"
  );
  assertEquals(
    stringify(async function af(): Promise<void> {}),
    "[AsyncFunction: af]"
  );
  assertEquals(
    stringify(function* gf() {}),
    "[GeneratorFunction: gf]"
  );
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
    "{ printFunc, log, debug, info, dir, dirxml, warn, error, assert, count, countReset, table, time, timeLog, timeEnd, group, groupCollapsed, groupEnd, clear, trace, indentLevel }"
  );
  // test inspect is working the same
  assertEquals(inspect(nestedObj), nestedObjExpected);
});
/* eslint-enable @typescript-eslint/explicit-function-return-type */

unitTest(function consoleTestStringifyWithDepth(): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const nestedObj: any = { a: { b: { c: { d: { e: { f: 42 } } } } } };
  assertEquals(
    stringifyArgs([nestedObj], { depth: 3 }),
    "{ a: { b: { c: [Object] } } }"
  );
  assertEquals(
    stringifyArgs([nestedObj], { depth: 4 }),
    "{ a: { b: { c: { d: [Object] } } } }"
  );
  assertEquals(stringifyArgs([nestedObj], { depth: 0 }), "[Object]");
  assertEquals(
    stringifyArgs([nestedObj]),
    "{ a: { b: { c: { d: [Object] } } } }"
  );
  // test inspect is working the same way
  assertEquals(
    inspect(nestedObj, { depth: 4 }),
    "{ a: { b: { c: { d: [Object] } } } }"
  );
});

unitTest(function consoleTestWithCustomInspector(): void {
  class A {
    [customInspect](): string {
      return "b";
    }
  }

  assertEquals(stringify(new A()), "b");
});

unitTest(function consoleTestWithCustomInspectorError(): void {
  class A {
    [customInspect](): string {
      throw new Error("BOOM");
      return "b";
    }
  }

  assertEquals(stringify(new A()), "A {}");

  class B {
    constructor(public field: { a: string }) {}
    [customInspect](): string {
      return this.field.a;
    }
  }

  assertEquals(stringify(new B({ a: "a" })), "a");
  assertEquals(stringify(B.prototype), "{}");
});

unitTest(function consoleTestWithIntegerFormatSpecifier(): void {
  assertEquals(stringify("%i"), "%i");
  assertEquals(stringify("%i", 42.0), "42");
  assertEquals(stringify("%i", 42), "42");
  assertEquals(stringify("%i", "42"), "42");
  assertEquals(stringify("%i", "42.0"), "42");
  assertEquals(stringify("%i", 1.5), "1");
  assertEquals(stringify("%i", -0.5), "-0");
  assertEquals(stringify("%i", ""), "NaN");
  assertEquals(stringify("%i", Symbol()), "NaN");
  assertEquals(stringify("%i %d", 42, 43), "42 43");
  assertEquals(stringify("%d %i", 42), "42 %i");
  assertEquals(stringify("%i", 12345678901234567890123), "1");
  assertEquals(
    stringify("%i", 12345678901234567890123n),
    "12345678901234567890123n"
  );
});

unitTest(function consoleTestWithFloatFormatSpecifier(): void {
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

unitTest(function consoleTestWithStringFormatSpecifier(): void {
  assertEquals(stringify("%s"), "%s");
  assertEquals(stringify("%s", undefined), "undefined");
  assertEquals(stringify("%s", "foo"), "foo");
  assertEquals(stringify("%s", 42), "42");
  assertEquals(stringify("%s", "42"), "42");
  assertEquals(stringify("%s %s", 42, 43), "42 43");
  assertEquals(stringify("%s %s", 42), "42 %s");
  assertEquals(stringify("%s", Symbol("foo")), "Symbol(foo)");
});

unitTest(function consoleTestWithObjectFormatSpecifier(): void {
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

unitTest(function consoleTestWithVariousOrInvalidFormatSpecifier(): void {
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

unitTest(function consoleTestCallToStringOnLabel(): void {
  const methods = ["count", "countReset", "time", "timeLog", "timeEnd"];
  mockConsole(console => {
    for (const method of methods) {
      let hasCalled = false;
      // @ts-ignore
      console[method]({
        toString(): void {
          hasCalled = true;
        }
      });
      assertEquals(hasCalled, true);
    }
  });
});

unitTest(function consoleTestError(): void {
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

unitTest(function consoleTestClear(): void {
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
  assertEquals(buffer, uint8);
});

// Test bound this issue
unitTest(function consoleDetachedLog(): void {
  mockConsole(console => {
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
unitTest(function consoleGroup(): void {
  mockConsole((console, out): void => {
    console.group("1");
    console.log("2");
    console.group("3");
    console.log("4");
    console.groupEnd();
    console.groupEnd();
    console.log("5");
    console.log("6");

    assertEquals(
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
unitTest(function consoleGroupWarn(): void {
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
    assertEquals(
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
unitTest(function consoleTable(): void {
  mockConsole((console, out): void => {
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
  mockConsole((console, out): void => {
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
  mockConsole((console, out): void => {
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
  mockConsole((console, out): void => {
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
  mockConsole((console, out): void => {
    console.table(
      new Map([
        [1, "one"],
        [2, "two"]
      ])
    );
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
  mockConsole((console, out): void => {
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
  mockConsole((console, out): void => {
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
  mockConsole((console, out): void => {
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
  mockConsole((console, out): void => {
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
  mockConsole((console, out): void => {
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
  mockConsole((console, out): void => {
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
  mockConsole((console, out): void => {
    console.table("test");
    assertEquals(out.toString(), "test\n");
  });
});

// console.log(Error) test
unitTest(function consoleLogShouldNotThrowError(): void {
  mockConsole(console => {
    let result = 0;
    try {
      console.log(new Error("foo"));
      result = 1;
    } catch (e) {
      result = 2;
    }
    assertEquals(result, 1);
  });

  // output errors to the console should not include "Uncaught"
  mockConsole((console, out): void => {
    console.log(new Error("foo"));
    assertEquals(out.toString().includes("Uncaught"), false);
  });
});

// console.dir test
unitTest(function consoleDir(): void {
  mockConsole((console, out): void => {
    console.dir("DIR");
    assertEquals(out.toString(), "DIR\n");
  });
  mockConsole((console, out): void => {
    console.dir("DIR", { indentLevel: 2 });
    assertEquals(out.toString(), "  DIR\n");
  });
});

// console.dir test
unitTest(function consoleDirXml(): void {
  mockConsole((console, out): void => {
    console.dirxml("DIRXML");
    assertEquals(out.toString(), "DIRXML\n");
  });
  mockConsole((console, out): void => {
    console.dirxml("DIRXML", { indentLevel: 2 });
    assertEquals(out.toString(), "  DIRXML\n");
  });
});

// console.trace test
unitTest(function consoleTrace(): void {
  mockConsole((console, _out, err): void => {
    console.trace("%s", "custom message");
    assert(err);
    assert(err.toString().includes("Trace: custom message"));
  });
});

unitTest(function consoleTestSymbols(): void {
  mockConsole((console, out): void => {
    console.log({ [Symbol.iterator]: "a", a: "a" });
    assert(out.toString(), "{ a: 'a', [Symbol(Symbol.iterator)]: 'a' }");
  });
});

unitTest(function consoleTestObjectWithManyAttributes(): void {
  mockConsole((console, out): void => {
    const obj = {
      a: "a",
      b: "b",
      c: "c",
      d: "d",
      e: "e",
      f: "f"
    };
    console.log(obj);
    assert(
      out.toString(),
      "{ a: 'a', b: 'b', c: 'c', d: 'd', e: 'e', f: 'f' }"
    );
  });
});

unitTest(function consoleTestLongArray(): void {
  mockConsole((console, out): void => {
    const array = [
      0,
      1,
      2,
      3,
      4,
      5,
      6,
      7,
      8,
      9,
      10,
      11,
      12,
      13,
      14,
      15,
      16,
      17,
      18,
      19,
      20,
      21,
      22,
      23,
      24,
      25,
      26,
      27,
      28,
      29,
      30,
      31,
      32,
      33,
      34,
      35,
      36,
      37,
      38,
      39,
      40,
      41,
      42,
      43,
      44,
      45,
      46,
      47,
      48,
      49,
      50,
      51,
      52,
      53,
      54,
      55,
      56,
      57,
      58,
      59,
      60,
      61,
      62,
      63,
      64,
      65,
      66,
      67,
      68,
      69,
      70,
      71,
      72,
      73,
      74,
      75,
      76,
      77,
      78,
      79,
      80,
      81,
      82,
      83,
      84,
      85,
      86,
      87,
      88,
      89,
      90,
      91,
      92,
      93,
      94,
      95,
      96,
      97,
      98,
      99
    ];
    console.log(array);
    const expected =
      "[\n" +
      "   0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11,\n" +
      "  12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,\n" +
      "  24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35,\n" +
      "  36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,\n" +
      "  48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59,\n" +
      "  60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71,\n" +
      "  72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83,\n" +
      "  84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95,\n" +
      "  96, 97, 98, 99\n" +
      "]";
    assert(out.toString(), expected);
  });
});


unitTest(function nodeJsTests(){
  assertEquals(stringify(), '');
assertEquals(stringify(''), '');
assertEquals(stringify([]), '[]');
assertEquals(stringify([0]), '[ 0 ]');
assertEquals(stringify({}), '{}');
assertEquals(stringify({ foo: 42 }), '{ foo: 42 }');
assertEquals(stringify(null), 'null');
assertEquals(stringify(true), 'true');
assertEquals(stringify(false), 'false');
assertEquals(stringify('test'), 'test');

// CHECKME this is for console.log() compatibility - but is it *right*?
assertEquals(stringify('foo', 'bar', 'baz'), 'foo bar baz');

// ES6 Symbol handling
const symbol = Symbol("foo");
assertEquals(stringify(symbol), 'Symbol(foo)');
assertEquals(stringify('foo', symbol), 'foo Symbol(foo)');
assertEquals(stringify('%s', symbol), 'Symbol(foo)');
assertEquals(stringify('%j', symbol), 'undefined');

// Number format specifier
assertEquals(stringify('%d'), '%d');
assertEquals(stringify('%d', 42.0), '42');
assertEquals(stringify('%d', 42), '42');
assertEquals(stringify('%d', '42'), '42');
assertEquals(stringify('%d', '42.0'), '42');
assertEquals(stringify('%d', 1.5), '1.5');
assertEquals(stringify('%d', -0.5), '-0.5');
assertEquals(stringify('%d', -0.0), '-0');
assertEquals(stringify('%d', ''), '0');
assertEquals(stringify('%d', ' -0.000'), '-0');
assertEquals(stringify('%d', Symbol()), 'NaN');
assertEquals(stringify('%d %d', 42, 43), '42 43');
assertEquals(stringify('%d %d', 42), '42 %d');
assertEquals(
  stringify('%d', 1180591620717411303424),
  '1.1805916207174113e+21'
);
assertEquals(
  stringify('%d', 1180591620717411303424n),
  '1180591620717411303424n'
);
assertEquals(
  stringify('%d %d', 1180591620717411303424n, 12345678901234567890123n),
  '1180591620717411303424n 12345678901234567890123n'
);

// Integer format specifier
assertEquals(stringify('%i'), '%i');
assertEquals(stringify('%i', 42.0), '42');
assertEquals(stringify('%i', 42), '42');
assertEquals(stringify('%i', '42'), '42');
assertEquals(stringify('%i', '42.0'), '42');
assertEquals(stringify('%i', 1.5), '1');
assertEquals(stringify('%i', -0.5), '-0');
assertEquals(stringify('%i', ''), 'NaN');
assertEquals(stringify('%i', Symbol()), 'NaN');
assertEquals(stringify('%i %i', 42, 43), '42 43');
assertEquals(stringify('%i %i', 42), '42 %i');
assertEquals(
  stringify('%i', 1180591620717411303424),
  '1'
);
assertEquals(
  stringify('%i', 1180591620717411303424n),
  '1180591620717411303424n'
);
assertEquals(
  stringify('%i %i', 1180591620717411303424n, 12345678901234567890123n),
  '1180591620717411303424n 12345678901234567890123n'
);

assertEquals(
  stringify('%d %i', 1180591620717411303424n, 12345678901234567890123n),
  '1180591620717411303424n 12345678901234567890123n'
);

assertEquals(
  stringify('%i %d', 1180591620717411303424n, 12345678901234567890123n),
  '1180591620717411303424n 12345678901234567890123n'
);

// Float format specifier
assertEquals(stringify('%f'), '%f');
assertEquals(stringify('%f', 42.0), '42');
assertEquals(stringify('%f', 42), '42');
assertEquals(stringify('%f', '42'), '42');
assertEquals(stringify('%f', '-0.0'), '-0');
assertEquals(stringify('%f', '42.0'), '42');
assertEquals(stringify('%f', 1.5), '1.5');
assertEquals(stringify('%f', -0.5), '-0.5');
assertEquals(stringify('%f', Math.PI), '3.141592653589793');
assertEquals(stringify('%f', ''), 'NaN');
assertEquals(stringify('%f', Symbol('foo')), 'NaN');
assertEquals(stringify('%f', 5n), '5');
assertEquals(stringify('%f %f', 42, 43), '42 43');
assertEquals(stringify('%f %f', 42), '42 %f');

// String format specifier
assertEquals(stringify('%s'), '%s');
assertEquals(stringify('%s', undefined), 'undefined');
assertEquals(stringify('%s', null), 'null');
assertEquals(stringify('%s', 'foo'), 'foo');
assertEquals(stringify('%s', 42), '42');
assertEquals(stringify('%s', '42'), '42');
assertEquals(stringify('%s', -0), '-0');
assertEquals(stringify('%s', '-0.0'), '-0.0');
assertEquals(stringify('%s %s', 42, 43), '42 43');
assertEquals(stringify('%s %s', 42), '42 %s');
assertEquals(stringify('%s', 42n), '42n');
assertEquals(stringify('%s', Symbol('foo')), 'Symbol(foo)');
assertEquals(stringify('%s', true), 'true');
assertEquals(stringify('%s', { a: [1, 2, 3] }), '{ a: [Array] }');
assertEquals(stringify('%s', { toString() { return 'Foo'; } }), 'Foo');
assertEquals(stringify('%s', { toString: 5 }), '{ toString: 5 }');
assertEquals(stringify('%s', () => 5), '() => 5');

// String format specifier including `toString` properties on the prototype.
{
  class Foo { toString() { return 'Bar'; } }
  assertEquals(stringify('%s', new Foo()), 'Bar');
  assertEquals(
    stringify('%s', Object.setPrototypeOf(new Foo(), null)),
    '[Foo: null prototype] {}'
  );
  window["Foo"] = Foo;
  assertEquals(stringify('%s', new Foo()), 'Bar');
  delete window["Foo"];
  class Bar { abc = true; }
  assertEquals(stringify('%s', new Bar()), 'Bar { abc: true }');
  class Foobar extends Array { aaa = true; }
  assertEquals(
    stringify('%s', new Foobar(5)),
    'Foobar(5) [ <5 empty items>, aaa: true ]'
  );

  // Subclassing:
  class B extends Foo {}

  function C() {}
  C.prototype.toString = function() {
    return 'Custom';
  };

  function D() {
    C.call(this);
  }
  D.prototype = Object.create(C.prototype);

  assertEquals(
    stringify('%s', new B()),
    'Bar'
  );
  assertEquals(
    stringify('%s', new C()),
    'Custom'
  );
  assertEquals(
    stringify('%s', new D()),
    'Custom'
  );

  D.prototype.constructor = D;
  assertEquals(
    stringify('%s', new D()),
    'Custom'
  );

  D.prototype.constructor = null;
  assertEquals(
    stringify('%s', new D()),
    'Custom'
  );

  D.prototype.constructor = { name: 'Foobar' };
  assertEquals(
    stringify('%s', new D()),
    'Custom'
  );

  Object.defineProperty(D.prototype, 'constructor', {
    get() {
      throw new Error();
    },
    configurable: true
  });
  assertEquals(
    stringify('%s', new D()),
    'Custom'
  );

  assertEquals(
    stringify('%s', Object.create(null)),
    '[Object: null prototype] {}'
  );
}

// JSON format specifier
assertEquals(stringify('%j'), '%j');
assertEquals(stringify('%j', 42), '42');
assertEquals(stringify('%j', '42'), '"42"');
assertEquals(stringify('%j %j', 42, 43), '42 43');
assertEquals(stringify('%j %j', 42), '42 %j');

// Object format specifier
const obj = {
  foo: 'bar',
  foobar: 1,
  func: function() {}
};
const nestedObj = {
  foo: 'bar',
  foobar: {
    foo: 'bar',
    func: function() {}
  }
};
const nestedObj2 = {
  foo: 'bar',
  foobar: 1,
  func: [{ a: function() {} }]
};
assertEquals(stringify('%o'), '%o');
assertEquals(stringify('%o', 42), '42');
assertEquals(stringify('%o', 'foo'), '\'foo\'');
assertEquals(
  stringify('%o', obj),
  '{\n' +
  '  foo: \'bar\',\n' +
  '  foobar: 1,\n' +
  '  func: <ref *1> [Function: func] {\n' +
  '    [length]: 0,\n' +
  '    [name]: \'func\',\n' +
  '    [prototype]: func { [constructor]: [Circular *1] }\n' +
  '  }\n' +
  '}');
assertEquals(
  stringify('%o', nestedObj2),
  '{\n' +
  '  foo: \'bar\',\n' +
  '  foobar: 1,\n' +
  '  func: [\n' +
  '    {\n' +
  '      a: <ref *1> [Function: a] {\n' +
  '        [length]: 0,\n' +
  '        [name]: \'a\',\n' +
  '        [prototype]: a { [constructor]: [Circular *1] }\n' +
  '      }\n' +
  '    },\n' +
  '    [length]: 1\n' +
  '  ]\n' +
  '}');
assertEquals(
  stringify('%o', nestedObj),
  '{\n' +
  '  foo: \'bar\',\n' +
  '  foobar: {\n' +
  '    foo: \'bar\',\n' +
  '    func: <ref *1> [Function: func] {\n' +
  '      [length]: 0,\n' +
  '      [name]: \'func\',\n' +
  '      [prototype]: func { [constructor]: [Circular *1] }\n' +
  '    }\n' +
  '  }\n' +
  '}');
assertEquals(
  stringify('%o %o', obj, obj),
  '{\n' +
  '  foo: \'bar\',\n' +
  '  foobar: 1,\n' +
  '  func: <ref *1> [Function: func] {\n' +
  '    [length]: 0,\n' +
  '    [name]: \'func\',\n' +
  '    [prototype]: func { [constructor]: [Circular *1] }\n' +
  '  }\n' +
  '} {\n' +
  '  foo: \'bar\',\n' +
  '  foobar: 1,\n' +
  '  func: <ref *1> [Function: func] {\n' +
  '    [length]: 0,\n' +
  '    [name]: \'func\',\n' +
  '    [prototype]: func { [constructor]: [Circular *1] }\n' +
  '  }\n' +
  '}');
assertEquals(
  stringify('%o %o', obj),
  '{\n' +
  '  foo: \'bar\',\n' +
  '  foobar: 1,\n' +
  '  func: <ref *1> [Function: func] {\n' +
  '    [length]: 0,\n' +
  '    [name]: \'func\',\n' +
  '    [prototype]: func { [constructor]: [Circular *1] }\n' +
  '  }\n' +
  '} %o');

assertEquals(stringify('%O'), '%O');
assertEquals(stringify('%O', 42), '42');
assertEquals(stringify('%O', 'foo'), '\'foo\'');
assertEquals(
  stringify('%O', obj),
  '{ foo: \'bar\', foobar: 1, func: [Function: func] }');
assertEquals(
  stringify('%O', nestedObj),
  '{ foo: \'bar\', foobar: { foo: \'bar\', func: [Function: func] } }');
assertEquals(
  stringify('%O %O', obj, obj),
  '{ foo: \'bar\', foobar: 1, func: [Function: func] } ' +
  '{ foo: \'bar\', foobar: 1, func: [Function: func] }');
assertEquals(
  stringify('%O %O', obj),
  '{ foo: \'bar\', foobar: 1, func: [Function: func] } %O');

// Various format specifiers
assertEquals(stringify('%%s%s', 'foo'), '%sfoo');
assertEquals(stringify('%s:%s'), '%s:%s');
assertEquals(stringify('%s:%s', undefined), 'undefined:%s');
assertEquals(stringify('%s:%s', 'foo'), 'foo:%s');
assertEquals(stringify('%s:%i', 'foo'), 'foo:%i');
assertEquals(stringify('%s:%f', 'foo'), 'foo:%f');
assertEquals(stringify('%s:%s', 'foo', 'bar'), 'foo:bar');
assertEquals(stringify('%s:%s', 'foo', 'bar', 'baz'), 'foo:bar baz');
assertEquals(stringify('%%%s%%', 'hi'), '%hi%');
assertEquals(stringify('%%%s%%%%', 'hi'), '%hi%%');
assertEquals(stringify('%sbc%%def', 'a'), 'abc%def');
assertEquals(stringify('%d:%d', 12, 30), '12:30');
assertEquals(stringify('%d:%d', 12), '12:%d');
assertEquals(stringify('%d:%d'), '%d:%d');
assertEquals(stringify('%i:%i', 12, 30), '12:30');
assertEquals(stringify('%i:%i', 12), '12:%i');
assertEquals(stringify('%i:%i'), '%i:%i');
assertEquals(stringify('%f:%f', 12, 30), '12:30');
assertEquals(stringify('%f:%f', 12), '12:%f');
assertEquals(stringify('%f:%f'), '%f:%f');
assertEquals(stringify('o: %j, a: %j', {}, []), 'o: {}, a: []');
assertEquals(stringify('o: %j, a: %j', {}), 'o: {}, a: %j');
assertEquals(stringify('o: %j, a: %j'), 'o: %j, a: %j');
assertEquals(stringify('o: %o, a: %O', {}, []), 'o: {}, a: []');
assertEquals(stringify('o: %o, a: %o', {}), 'o: {}, a: %o');
assertEquals(stringify('o: %O, a: %O'), 'o: %O, a: %O');


// Invalid format specifiers
assertEquals(stringify('a% b', 'x'), 'a% b x');
assertEquals(stringify('percent: %d%, fraction: %d', 10, 0.1),
                   'percent: 10%, fraction: 0.1');
assertEquals(stringify('abc%', 1), 'abc% 1');

// Additional arguments after format specifiers
assertEquals(stringify('%i', 1, 'number'), '1 number');
assertEquals(stringify('%i', 1, () => {}), '1 [Function (anonymous)]');

// %c from https://console.spec.whatwg.org/
assertEquals(stringify('%c'), '%c');
assertEquals(stringify('%cab'), '%cab');
assertEquals(stringify('%cab', 'color: blue'), 'ab');
assertEquals(stringify('%cab', 'color: blue', 'c'), 'ab c');

{
  const o = {};
  o["o"] = o;
  assertEquals(stringify('%j', o), '[Circular]');
}

{
  const o = {
    toJSON() {
      throw new Error('Not a circular object but still not serializable');
    }
  };
  assertThrows(() => stringify('%j', o),
                Error,
                "Not a circular object but still not serializable");
}

// Errors
const err = new Error('foo');
assertEquals(stringify(err), err.stack);
class CustomError extends Error {
  constructor(msg) {
    super();
    Object.defineProperty(this, 'message',
                          { value: msg, enumerable: false });
    Object.defineProperty(this, 'name',
                          { value: 'CustomError', enumerable: false });
    Error["captureStackTrace"](this, CustomError);
  }
}
const customError = new CustomError('bar');
assertEquals(stringify(customError), customError.stack);
// Doesn't capture stack trace
function BadCustomError(msg) {
  Error.call(this);
  Object.defineProperty(this, 'message',
                        { value: msg, enumerable: false });
  Object.defineProperty(this, 'name',
                        { value: 'BadCustomError', enumerable: false });
}
Object.setPrototypeOf(BadCustomError.prototype, Error.prototype);
Object.setPrototypeOf(BadCustomError, Error);
assertEquals(stringify(new BadCustomError('foo')),
                   '[BadCustomError: foo]');

// The format of arguments should not depend on type of the first argument
assertEquals(stringify('1', '1'), '1 1');
assertEquals(stringify(1, '1'), '1 1');
assertEquals(stringify('1', 1), '1 1');
assertEquals(stringify(1, -0), '1 -0');
assertEquals(stringify('1', () => {}), '1 [Function (anonymous)]');
assertEquals(stringify(1, () => {}), '1 [Function (anonymous)]');
assertEquals(stringify('1', "'"), "1 '");
assertEquals(stringify(1, "'"), "1 '");
assertEquals(stringify('1', 'number'), '1 number');
assertEquals(stringify(1, 'number'), '1 number');
assertEquals(stringify(5n), '5n');
assertEquals(stringify(5n, 5n), '5n 5n');
});

Deno.runTests()