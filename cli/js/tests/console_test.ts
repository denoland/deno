// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, unitTest } from "./test_util.ts";

// Some of these APIs aren't exposed in the types and so we have to cast to any
// in order to "trick" TypeScript.
const {
  inspect,
  writeSync,
  stdout,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
} = Deno as any;

const customInspect = Deno.symbols.customInspect;
const {
  Console,
  stringifyArgs,
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
  mockConsole((console) => {
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
    extendedCstr: Extended,
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
    baseClass: new Base(),
  };

  nestedObj.o = circularObj;
  const nestedObjExpected = `{
 num: 1,
 bool: true,
 str: "a",
 method: [Function: method],
 asyncMethod: [AsyncFunction: asyncMethod],
 generatorMethod: [GeneratorFunction: generatorMethod],
 un: undefined,
 nu: null,
 arrowFunc: [Function: arrowFunc],
 extendedClass: Extended { a: 1, b: 2 },
 nFunc: [Function],
 extendedCstr: [Function: Extended],
 o: {
  num: 2,
  bool: false,
  str: "b",
  method: [Function: method],
  un: undefined,
  nu: null,
  nested: [Circular],
  emptyObj: {},
  arr: [ 1, "s", false, null, [Circular] ],
  baseClass: Base { a: 1 }
 }
}`;

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
        [2, "two"],
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
  assertEquals(stringify(JSON), 'JSON { Symbol(Symbol.toStringTag): "JSON" }');
  assertEquals(
    stringify(console),
    `{
 log: [Function],
 debug: [Function],
 info: [Function],
 dir: [Function],
 dirxml: [Function],
 warn: [Function],
 error: [Function],
 assert: [Function],
 count: [Function],
 countReset: [Function],
 table: [Function],
 time: [Function],
 timeLog: [Function],
 timeEnd: [Function],
 group: [Function],
 groupCollapsed: [Function],
 groupEnd: [Function],
 clear: [Function],
 trace: [Function],
 indentLevel: 0,
 Symbol(isConsoleInstance): true
}`
  );
  assertEquals(
    stringify({ str: 1, [Symbol.for("sym")]: 2, [Symbol.toStringTag]: "TAG" }),
    'TAG { str: 1, Symbol(sym): 2, Symbol(Symbol.toStringTag): "TAG" }'
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

unitTest(function consoleTestStringifyLargeObject(): void {
  const obj = {
    a: 2,
    o: {
      a: "1",
      b: "2",
      c: "3",
      d: "4",
      e: "5",
      f: "6",
      g: 10,
      asd: 2,
      asda: 3,
      x: { a: "asd", x: 3 },
    },
  };
  assertEquals(
    stringify(obj),
    `{
 a: 2,
 o: {
  a: "1",
  b: "2",
  c: "3",
  d: "4",
  e: "5",
  f: "6",
  g: 10,
  asd: 2,
  asda: 3,
  x: { a: "asd", x: 3 }
 }
}`
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
  assertEquals(
    stringify(B.prototype),
    "{ Symbol(Deno.customInspect): [Function: [Deno.customInspect]] }"
  );
});

unitTest(function consoleTestWithIntegerFormatSpecifier(): void {
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
  mockConsole((console) => {
    for (const method of methods) {
      let hasCalled = false;
      // @ts-ignore
      console[method]({
        toString(): void {
          hasCalled = true;
        },
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
  mockConsole((console) => {
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
        [2, "two"],
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
      h: new Map([[1, "one"]]),
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
      ["test", { b: 20, c: "test" }],
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
  mockConsole((console) => {
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
