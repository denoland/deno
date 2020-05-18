// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// TODO(ry) The test functions in this file have way too many asserts in them.
// They need to be broken up into smaller more manageable tests.

import { assert, assertEquals, unitTest } from "./test_util.ts";
import {
  stripColor,
  yellow,
  cyan,
  red,
  green,
  magenta,
  bold,
  dim,
} from "../../../std/fmt/colors.ts";

// Some of these APIs aren't exposed in the types and so we have to cast to any
// in order to "trick" TypeScript.
const {
  inspect,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
} = Deno as any;

const customInspect = Deno.customInspect;
const {
  Console,
  stringifyArgs,
  // @ts-ignore TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

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

// Modifying the test to take colors into account
unitTest(function consoleTestStringifyComplexObjects(): void {
  assertEquals(stringify("foo"), "foo");
  assertEquals(stripColor(stringify(["foo", "bar"])), `[ "foo", "bar" ]`);
  assertEquals(stripColor(stringify({ foo: "bar" })), `{ foo: "bar" }`);
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
  num: ${yellow("1")},
  bool: ${yellow("true")},
  str: ${green('"a"')},
  method: ${cyan("[Function: method]")},
  asyncMethod: ${cyan("[AsyncFunction: asyncMethod]")},
  generatorMethod: ${cyan("[GeneratorFunction: generatorMethod]")},
  un: ${dim("undefined")},
  nu: ${bold("null")},
  arrowFunc: ${cyan("[Function: arrowFunc]")},
  extendedClass: Extended { a: ${yellow("1")}, b: ${yellow("2")} },
  nFunc: ${cyan("[Function]")},
  extendedCstr: ${cyan("[Function: Extended]")},
  o: {
    num: ${yellow("2")},
    bool: ${yellow("false")},
    str: ${green('"b"')},
    method: ${cyan("[Function: method]")},
    un: ${dim("undefined")},
    nu: ${bold("null")},
    nested: ${cyan("[Circular]")},
    emptyObj: {},
    arr: [ ${yellow("1")}, ${green('"s"')}, ${yellow("false")}, ${bold("null")}, ${cyan("[Circular]")} ],
    baseClass: Base { a: ${yellow("1")} }
  }
}`;

  assertEquals(stringify(1), yellow("1"));
  assertEquals(stringify(-0), yellow("-0"));
  assertEquals(stringify(1n), yellow("1n"));
  assertEquals(stringify("s"), "s");
  assertEquals(stringify(false), yellow("false"));
  assertEquals(stringify(new Number(1)), cyan("[Number: 1]"));
  assertEquals(stringify(new Boolean(true)), cyan("[Boolean: true]"));
  assertEquals(stringify(new String("deno")), cyan(`[String: "deno"]`));
  assertEquals(stringify(/[0-9]*/), red("/[0-9]*/"));
  assertEquals(
    stringify(new Date("2018-12-10T02:26:59.002Z")),
    magenta("2018-12-10T02:26:59.002Z")
  );
  assertEquals(
    stringify(new Set([1, 2, 3])),
    `Set { ${[1, 2, 3].map((n) => yellow(String(n))).join(", ")} }`
  );
  assertEquals(
    stringify(
      new Map([
        [1, "one"],
        [2, "two"],
      ])
    ),
    `Map { ${yellow("1")} => ${green('"one"')}, ${yellow("2")} => ${green(
      '"two"'
    )} }`
  );
  assertEquals(
    stringify(new WeakSet()),
    `WeakSet { ${cyan("[items unknown]")} }`
  );
  assertEquals(
    stringify(new WeakMap()),
    `WeakMap { ${cyan("[items unknown]")} }`
  );
  assertEquals(stringify(Symbol(1)), green("Symbol(1)"));
  assertEquals(stringify(null), bold("null"));
  assertEquals(stringify(undefined), dim("undefined"));
  assertEquals(
    stringify(new Extended()),
    `Extended { a: ${yellow("1")}, b: ${yellow("2")} }`
  );
  assertEquals(
    stringify(function f(): void {}),
    cyan("[Function: f]")
  );
  assertEquals(
    stringify(async function af(): Promise<void> {}),
    cyan("[AsyncFunction: af]")
  );
  assertEquals(
    stringify(function* gf() {}),
    cyan("[GeneratorFunction: gf]")
  );
  assertEquals(
    stringify(async function* agf() {}),
    cyan("[AsyncGeneratorFunction: agf]")
  );
  assertEquals(
    stringify(new Uint8Array([1, 2, 3])),
    `Uint8Array(3) [ ${[1, 2, 3].map((n) => yellow(String(n))).join(", ")} ]`
  );
  assertEquals(stringify(Uint8Array.prototype), "TypedArray {}");
  assertEquals(
    stringify({ a: { b: { c: { d: new Set([1]) } } } }),
    `{ a: { b: { c: { d: ${cyan("[Set]")} } } } }`
  );
  assertEquals(stringify(nestedObj), nestedObjExpected);
  assertEquals(
    stripColor(stringify(JSON)),
    `JSON { Symbol(Symbol.toStringTag): ${green("JSON")} }`
  );
  assertEquals(
    stringify(console),
    `{
  log: ${cyan("[Function]")},
  debug: ${cyan("[Function]")},
  info: ${cyan("[Function]")},
  dir: ${cyan("[Function]")},
  dirxml: ${cyan("[Function]")},
  warn: ${cyan("[Function]")},
  error: ${cyan("[Function]")},
  assert: ${cyan("[Function]")},
  count: ${cyan("[Function]")},
  countReset: ${cyan("[Function]")},
  table: ${cyan("[Function]")},
  time: ${cyan("[Function]")},
  timeLog: ${cyan("[Function]")},
  timeEnd: ${cyan("[Function]")},
  group: ${cyan("[Function]")},
  groupCollapsed: ${cyan("[Function]")},
  groupEnd: ${cyan("[Function]")},
  clear: ${cyan("[Function]")},
  trace: ${cyan("[Function]")},
  indentLevel: ${yellow("0")},
  Symbol(isConsoleInstance): ${yellow("true")}
}`
  );
  assertEquals(
    stringify({ str: 1, [Symbol.for("sym")]: 2, [Symbol.toStringTag]: "TAG" }),
    `TAG { str: ${yellow("1")}, Symbol(sym): ${yellow("2")}, Symbol(Symbol.toStringTag): ${green('"TAG"')} }`
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
    `{ a: { b: { c: ${cyan("[Object]")} } } }`
  );
  assertEquals(
    stringifyArgs([nestedObj], { depth: 4 }),
    `{ a: { b: { c: { d: ${cyan("[Object]")} } } } }`
  );
  assertEquals(stringifyArgs([nestedObj], { depth: 0 }), cyan("[Object]"));
  assertEquals(
    stringifyArgs([nestedObj]),
    `{ a: { b: { c: { d: ${cyan("[Object]")} } } } }`
  );
  // test inspect is working the same way
  assertEquals(
    inspect(nestedObj, { depth: 4 }),
    `{ a: { b: { c: { d: ${cyan("[Object]")} } } } }`
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
    stripColor(stringify(obj)),
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

unitTest(function consoleTestStringifyIterable() {
  const shortArray = [1, 2, 3];
  assertEquals(
    stringify(shortArray),
    `[ ${[1, 2, 3].map((n) => yellow(String(n))).join(", ")} ]`
  );

  const longArray = new Array(200).fill(0);
  assertEquals(
    stripColor(stringify(longArray)),
    `[
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  ... 100 more items
]`
  );

  const obj = { a: "a", longArray };
  assertEquals(
    stripColor(stringify(obj)),
    `{
  a: "a",
  longArray: [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ... 100 more items
  ]
}`
  );

  const shortMap = new Map([
    ["a", 0],
    ["b", 1],
  ]);
  assertEquals(
    stringify(shortMap),
    `Map { ${green('"a"')} => ${yellow("0")}, ${green('"b"')} => ${yellow(
      "1"
    )} }`
  );

  const longMap = new Map();
  for (const key of Array(200).keys()) {
    longMap.set(`${key}`, key);
  }
  assertEquals(
    stripColor(stringify(longMap)),
    `Map {
  "0" => 0,
  "1" => 1,
  "2" => 2,
  "3" => 3,
  "4" => 4,
  "5" => 5,
  "6" => 6,
  "7" => 7,
  "8" => 8,
  "9" => 9,
  "10" => 10,
  "11" => 11,
  "12" => 12,
  "13" => 13,
  "14" => 14,
  "15" => 15,
  "16" => 16,
  "17" => 17,
  "18" => 18,
  "19" => 19,
  "20" => 20,
  "21" => 21,
  "22" => 22,
  "23" => 23,
  "24" => 24,
  "25" => 25,
  "26" => 26,
  "27" => 27,
  "28" => 28,
  "29" => 29,
  "30" => 30,
  "31" => 31,
  "32" => 32,
  "33" => 33,
  "34" => 34,
  "35" => 35,
  "36" => 36,
  "37" => 37,
  "38" => 38,
  "39" => 39,
  "40" => 40,
  "41" => 41,
  "42" => 42,
  "43" => 43,
  "44" => 44,
  "45" => 45,
  "46" => 46,
  "47" => 47,
  "48" => 48,
  "49" => 49,
  "50" => 50,
  "51" => 51,
  "52" => 52,
  "53" => 53,
  "54" => 54,
  "55" => 55,
  "56" => 56,
  "57" => 57,
  "58" => 58,
  "59" => 59,
  "60" => 60,
  "61" => 61,
  "62" => 62,
  "63" => 63,
  "64" => 64,
  "65" => 65,
  "66" => 66,
  "67" => 67,
  "68" => 68,
  "69" => 69,
  "70" => 70,
  "71" => 71,
  "72" => 72,
  "73" => 73,
  "74" => 74,
  "75" => 75,
  "76" => 76,
  "77" => 77,
  "78" => 78,
  "79" => 79,
  "80" => 80,
  "81" => 81,
  "82" => 82,
  "83" => 83,
  "84" => 84,
  "85" => 85,
  "86" => 86,
  "87" => 87,
  "88" => 88,
  "89" => 89,
  "90" => 90,
  "91" => 91,
  "92" => 92,
  "93" => 93,
  "94" => 94,
  "95" => 95,
  "96" => 96,
  "97" => 97,
  "98" => 98,
  "99" => 99,
  ... 100 more items
}`
  );

  const shortSet = new Set([1, 2, 3]);
  assertEquals(
    stringify(shortSet),
    `Set { ${[1, 2, 3].map((n) => yellow(String(n))).join(", ")} }`
  );
  const longSet = new Set();
  for (const key of Array(200).keys()) {
    longSet.add(key);
  }
  assertEquals(
    stripColor(stringify(longSet)),
    `Set {
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
  99,
  ... 100 more items
}`
  );

  const withEmptyEl = Array(10);
  withEmptyEl.fill(0, 4, 6);
  assertEquals(
    stringify(withEmptyEl),
    `[ ${dim("<4 empty items>")}, ${yellow("0")}, ${yellow("0")}, ${dim(
      "<4 empty items>"
    )} ]`
  );

  /* TODO(ry) Fix this test.
  const lWithEmptyEl = Array(200);
  lWithEmptyEl.fill(0, 50, 80);
  assertEquals(
    stripColor(stringify(lWithEmptyEl)),
    `[
  <50 empty items>, 0,                 0,
  0,                0,                 0,
  0,                0,                 0,
  0,                0,                 0,
  0,                0,                 0,
  0,                0,                 0,
  0,                0,                 0,
  0,                0,                 0,
  0,                0,                 0,
  0,                0,                 0,
  0,                <120 empty items>
]`
  );
  */
});

unitTest(async function consoleTestStringifyPromises(): Promise<void> {
  const pendingPromise = new Promise((_res, _rej) => {});
  assertEquals(stringify(pendingPromise), `Promise { ${cyan("<pending>")} }`);

  const resolvedPromise = new Promise((res, _rej) => {
    res("Resolved!");
  });
  assertEquals(
    stringify(resolvedPromise),
    `Promise { ${green('"Resolved!"')} }`
  );

  let rejectedPromise;
  try {
    rejectedPromise = new Promise((_, rej) => {
      rej(Error("Whoops"));
    });
    await rejectedPromise;
  } catch (err) {}
  const strLines = stringify(rejectedPromise).split("\n");
  assertEquals(strLines[0], "Promise {");
  assertEquals(strLines[1], ` ${red("<rejected>")} Error: Whoops`);
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
    `{ ${green("Symbol(Deno.symbols.customInspect)")}: ${cyan(
      "[Function: [Deno.symbols.customInspect]]"
    )} }`
  );
});

unitTest(function consoleTestWithIntegerFormatSpecifier(): void {
  assertEquals(stringify("%i"), "%i");
  assertEquals(stringify("%i", 42.0), yellow("42"));
  assertEquals(stringify("%i", 42), yellow("42"));
  assertEquals(stringify("%i", "42"), yellow("42"));
  assertEquals(stringify("%i", "42.0"), yellow("42"));
  assertEquals(stringify("%i", 1.5), yellow("1"));
  assertEquals(stringify("%i", -0.5), yellow("0"));
  assertEquals(stringify("%i", ""), yellow("NaN"));
  assertEquals(stringify("%i", Symbol()), yellow("NaN"));
  assertEquals(stringify("%i %d", 42, 43), `${yellow("42")} ${yellow("43")}`);
  assertEquals(stringify("%d %i", 42), `${yellow("42")} %i`);
  assertEquals(stringify("%d", 12345678901234567890123), yellow("1"));
  assertEquals(
    stringify("%i", 12345678901234567890123n),
    yellow("12345678901234567890123n")
  );
});

unitTest(function consoleTestWithFloatFormatSpecifier(): void {
  assertEquals(stringify("%f"), "%f");
  assertEquals(stringify("%f", 42.0), yellow("42"));
  assertEquals(stringify("%f", 42), yellow("42"));
  assertEquals(stringify("%f", "42"), yellow("42"));
  assertEquals(stringify("%f", "42.0"), yellow("42"));
  assertEquals(stringify("%f", 1.5), yellow("1.5"));
  assertEquals(stringify("%f", -0.5), yellow("-0.5"));
  assertEquals(stringify("%f", Math.PI), yellow("3.141592653589793"));
  assertEquals(stringify("%f", ""), yellow("NaN"));
  assertEquals(stringify("%f", Symbol("foo")), yellow("NaN"));
  assertEquals(stringify("%f", 5n), yellow("5"));
  assertEquals(stringify("%f %f", 42, 43), `${yellow("42")} ${yellow("43")}`);
  assertEquals(stringify("%f %f", 42), `${yellow("42")} %f`);
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
  assertEquals(stringify("%o", 42), yellow("42"));
  assertEquals(stringify("%o", "foo"), green("foo"));
  assertEquals(stringify("o: %o, a: %O", {}, []), "o: {}, a: []");
  assertEquals(stringify("%o", { a: 42 }), `{ a: ${yellow("42")} }`);
  assertEquals(
    stringify("%o", { a: { b: { c: { d: new Set([1]) } } } }),
    `{ a: { b: { c: { d: ${cyan("[Set]")} } } } }`
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
  assertEquals(stringify("%d:%d", 12), `${yellow("12")}:%d`);
  assertEquals(stringify("%i:%i", 12), `${yellow("12")}:%i`);
  assertEquals(stringify("%f:%f", 12), `${yellow("12")}:%f`);
  assertEquals(stringify("o: %o, a: %o", {}), "o: {}, a: %o");
  assertEquals(stringify("abc%", 1), `abc% ${yellow("1")}`);
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
  mockConsole((console, out) => {
    console.clear();
    assertEquals(out.toString(), "\x1b[1;1H" + "\x1b[0J");
  });
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
      stripColor(out.toString()),
      `┌───────┬────────┐
│ (idx) │ Values │
├───────┼────────┤
│   a   │ "test" │
│   b   │   1    │
└───────┴────────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table({ a: { b: 10 }, b: { b: 20, c: 30 } }, ["c"]);
    assertEquals(
      stripColor(out.toString()),
      `┌───────┬────┐
│ (idx) │ c  │
├───────┼────┤
│   a   │    │
│   b   │ 30 │
└───────┴────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table([1, 2, [3, [4]], [5, 6], [[7], [8]]]);
    assertEquals(
      stripColor(out.toString()),
      `┌───────┬───────┬───────┬────────┐
│ (idx) │   0   │   1   │ Values │
├───────┼───────┼───────┼────────┤
│   0   │       │       │   1    │
│   1   │       │       │   2    │
│   2   │   3   │ [ 4 ] │        │
│   3   │   5   │   6   │        │
│   4   │ [ 7 ] │ [ 8 ] │        │
└───────┴───────┴───────┴────────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table(new Set([1, 2, 3, "test"]));
    assertEquals(
      stripColor(out.toString()),
      `┌────────────┬────────┐
│ (iter idx) │ Values │
├────────────┼────────┤
│     0      │   1    │
│     1      │   2    │
│     2      │   3    │
│     3      │ "test" │
└────────────┴────────┘
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
      stripColor(out.toString()),
      `┌────────────┬─────┬────────┐
│ (iter idx) │ Key │ Values │
├────────────┼─────┼────────┤
│     0      │  1  │ "one"  │
│     1      │  2  │ "two"  │
└────────────┴─────┴────────┘
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
      stripColor(out.toString()),
      `┌───────┬───────────┬───────────────────┬────────┐
│ (idx) │     c     │         e         │ Values │
├───────┼───────────┼───────────────────┼────────┤
│   a   │           │                   │  true  │
│   b   │ { d: 10 } │ [ 1, 2, [Array] ] │        │
│   f   │           │                   │ "test" │
│   g   │           │                   │        │
│   h   │           │                   │        │
└───────┴───────────┴───────────────────┴────────┘
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
      stripColor(out.toString()),
      `┌───────┬────────┬──────────────────────┬────┬────────┐
│ (idx) │   0    │          1           │ a  │ Values │
├───────┼────────┼──────────────────────┼────┼────────┤
│   0   │        │                      │    │   1    │
│   1   │        │                      │    │ "test" │
│   2   │        │                      │    │ false  │
│   3   │        │                      │ 10 │        │
│   4   │ "test" │ { b: 20, c: "test" } │    │        │
└───────┴────────┴──────────────────────┴────┴────────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table([]);
    assertEquals(
      out.toString(),
      `┌───────┐
│ (idx) │
├───────┤
└───────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table({});
    assertEquals(
      out.toString(),
      `┌───────┐
│ (idx) │
├───────┤
└───────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table(new Set());
    assertEquals(
      out.toString(),
      `┌────────────┐
│ (iter idx) │
├────────────┤
└────────────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table(new Map());
    assertEquals(
      out.toString(),
      `┌────────────┐
│ (iter idx) │
├────────────┤
└────────────┘
`
    );
  });
  mockConsole((console, out): void => {
    console.table("test");
    assertEquals(out.toString(), "test\n");
  });
  mockConsole((console, out): void => {
    console.table(["Hello", "你好", "Amapá"]);
    assertEquals(
      stripColor(out.toString()),
      `┌───────┬─────────┐
│ (idx) │ Values  │
├───────┼─────────┤
│   0   │ "Hello" │
│   1   │ "你好"  │
│   2   │ "Amapá" │
└───────┴─────────┘
`
    );
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

// console.log(Invalid Date) test
unitTest(function consoleLogShoultNotThrowErrorWhenInvalidDateIsPassed(): void {
  mockConsole((console, out) => {
    const invalidDate = new Date("test");
    console.log(invalidDate);
    assertEquals(out.toString(), "Invalid Date\n");
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

console.log(stringify(JSON));
