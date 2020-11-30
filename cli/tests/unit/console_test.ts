// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// TODO(ry) The unit test functions in this module are too coarse. They should
// be broken up into smaller bits.

// TODO(ry) These tests currentl strip all the ANSI colors out. We don't have a
// good way to control whether we produce color output or not since
// std/fmt/colors auto determines whether to put colors in or not. We need
// better infrastructure here so we can properly test the colors.

import { stripColor } from "../../../std/fmt/colors.ts";
import {
  assert,
  assertEquals,
  assertStringIncludes,
  unitTest,
} from "./test_util.ts";

const customInspect = Deno.customInspect;
const {
  Console,
  cssToAnsi: cssToAnsi_,
  inspectArgs,
  parseCss: parseCss_,
  parseCssColor: parseCssColor_,
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

function stringify(...args: unknown[]): string {
  return stripColor(inspectArgs(args).replace(/\n$/, ""));
}

interface Css {
  backgroundColor: [number, number, number] | null;
  color: [number, number, number] | null;
  fontWeight: string | null;
  fontStyle: string | null;
  textDecorationColor: [number, number, number] | null;
  textDecorationLine: string[];
}

const DEFAULT_CSS: Css = {
  backgroundColor: null,
  color: null,
  fontWeight: null,
  fontStyle: null,
  textDecorationColor: null,
  textDecorationLine: [],
};

function parseCss(cssString: string): Css {
  return parseCss_(cssString);
}

function parseCssColor(colorString: string): Css {
  return parseCssColor_(colorString);
}

/** ANSI-fy the CSS, replace "\x1b" with "_". */
function cssToAnsiEsc(css: Css, prevCss: Css | null = null): string {
  return cssToAnsi_(css, prevCss).replaceAll("\x1b", "_");
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

unitTest(
  function consoleTestStringifyComplexObjectsWithEscapedSequences(): void {
    assertEquals(
      stringify(
        ["foo\b", "foo\f", "foo\n", "foo\r", "foo\t", "foo\v", "foo\0"],
      ),
      `[
  "foo\\b",   "foo\\f",
  "foo\\n",   "foo\\r",
  "foo\\t",   "foo\\v",
  "foo\\x00"
]`,
    );
    assertEquals(
      stringify(
        [
          Symbol(),
          Symbol(""),
          Symbol("foo\b"),
          Symbol("foo\f"),
          Symbol("foo\n"),
          Symbol("foo\r"),
          Symbol("foo\t"),
          Symbol("foo\v"),
          Symbol("foo\0"),
        ],
      ),
      `[
  Symbol(),
  Symbol(""),
  Symbol("foo\\b"),
  Symbol("foo\\f"),
  Symbol("foo\\n"),
  Symbol("foo\\r"),
  Symbol("foo\\t"),
  Symbol("foo\\v"),
  Symbol("foo\\x00")
]`,
    );
    assertEquals(
      stringify(
        { "foo\b": "bar\n", "bar\r": "baz\t", "qux\0": "qux\0" },
      ),
      `{ "foo\\b": "bar\\n", "bar\\r": "baz\\t", "qux\\x00": "qux\\x00" }`,
    );
    assertEquals(
      stringify(
        {
          [Symbol("foo\b")]: `Symbol("foo\n")`,
          [Symbol("bar\n")]: `Symbol("bar\n")`,
          [Symbol("bar\r")]: `Symbol("bar\r")`,
          [Symbol("baz\t")]: `Symbol("baz\t")`,
          [Symbol("qux\0")]: `Symbol("qux\0")`,
        },
      ),
      `{
  [Symbol("foo\\b")]: 'Symbol("foo\\n\")',
  [Symbol("bar\\n")]: 'Symbol("bar\\n\")',
  [Symbol("bar\\r")]: 'Symbol("bar\\r\")',
  [Symbol("baz\\t")]: 'Symbol("baz\\t\")',
  [Symbol("qux\\x00")]: 'Symbol(\"qux\\x00")'
}`,
    );
    assertEquals(
      stringify(new Set(["foo\n", "foo\r", "foo\0"])),
      `Set { "foo\\n", "foo\\r", "foo\\x00" }`,
    );
  },
);

unitTest(function consoleTestStringifyQuotes(): void {
  assertEquals(stringify(["\\"]), `[ "\\\\" ]`);
  assertEquals(stringify(['\\,"']), `[ '\\\\,"' ]`);
  assertEquals(stringify([`\\,",'`]), `[ \`\\\\,",'\` ]`);
  assertEquals(stringify(["\\,\",',`"]), `[ "\\\\,\\",',\`" ]`);
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

unitTest(function consoleTestStringifyCircular(): void {
  class Base {
    a = 1;
    m1() {}
  }

  class Extended extends Base {
    b = 2;
    m2() {}
  }

  // deno-lint-ignore no-explicit-any
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
    "2018-12-10T02:26:59.002Z",
  );
  assertEquals(stringify(new Set([1, 2, 3])), "Set { 1, 2, 3 }");
  assertEquals(
    stringify(
      new Map([
        [1, "one"],
        [2, "two"],
      ]),
    ),
    `Map { 1 => "one", 2 => "two" }`,
  );
  assertEquals(stringify(new WeakSet()), "WeakSet { [items unknown] }");
  assertEquals(stringify(new WeakMap()), "WeakMap { [items unknown] }");
  assertEquals(stringify(Symbol(1)), `Symbol("1")`);
  assertEquals(stringify(null), "null");
  assertEquals(stringify(undefined), "undefined");
  assertEquals(stringify(new Extended()), "Extended { a: 1, b: 2 }");
  assertEquals(
    stringify(function f(): void {}),
    "[Function: f]",
  );
  assertEquals(
    stringify(async function af(): Promise<void> {}),
    "[AsyncFunction: af]",
  );
  assertEquals(
    stringify(function* gf() {}),
    "[GeneratorFunction: gf]",
  );
  assertEquals(
    stringify(async function* agf() {}),
    "[AsyncGeneratorFunction: agf]",
  );
  assertEquals(
    stringify(new Uint8Array([1, 2, 3])),
    "Uint8Array(3) [ 1, 2, 3 ]",
  );
  assertEquals(stringify(Uint8Array.prototype), "Uint8Array {}");
  assertEquals(
    stringify({ a: { b: { c: { d: new Set([1]) } } } }),
    "{ a: { b: { c: { d: [Set] } } } }",
  );
  assertEquals(stringify(nestedObj), nestedObjExpected);
  assertEquals(
    stringify(JSON),
    'JSON { [Symbol(Symbol.toStringTag)]: "JSON" }',
  );
  assertEquals(
    stringify(console),
    `{
  log: [Function: log],
  debug: [Function: log],
  info: [Function: log],
  dir: [Function: dir],
  dirxml: [Function: dir],
  warn: [Function: warn],
  error: [Function: warn],
  assert: [Function: assert],
  count: [Function: count],
  countReset: [Function: countReset],
  table: [Function: table],
  time: [Function: time],
  timeLog: [Function: timeLog],
  timeEnd: [Function: timeEnd],
  group: [Function: group],
  groupCollapsed: [Function: group],
  groupEnd: [Function: groupEnd],
  clear: [Function: clear],
  trace: [Function: trace],
  indentLevel: 0,
  [Symbol(isConsoleInstance)]: true
}`,
  );
  assertEquals(
    stringify({ str: 1, [Symbol.for("sym")]: 2, [Symbol.toStringTag]: "TAG" }),
    'TAG { str: 1, [Symbol(sym)]: 2, [Symbol(Symbol.toStringTag)]: "TAG" }',
  );
  // test inspect is working the same
  assertEquals(stripColor(Deno.inspect(nestedObj)), nestedObjExpected);
});

unitTest(function consoleTestStringifyFunctionWithPrototypeRemoved(): void {
  const f = function f() {};
  Reflect.setPrototypeOf(f, null);
  assertEquals(stringify(f), "[Function: f]");
  const af = async function af() {};
  Reflect.setPrototypeOf(af, null);
  assertEquals(stringify(af), "[Function: af]");
  const gf = function* gf() {};
  Reflect.setPrototypeOf(gf, null);
  assertEquals(stringify(gf), "[Function: gf]");
  const agf = async function* agf() {};
  Reflect.setPrototypeOf(agf, null);
  assertEquals(stringify(agf), "[Function: agf]");
});

unitTest(function consoleTestStringifyWithDepth(): void {
  // deno-lint-ignore no-explicit-any
  const nestedObj: any = { a: { b: { c: { d: { e: { f: 42 } } } } } };
  assertEquals(
    stripColor(inspectArgs([nestedObj], { depth: 3 })),
    "{ a: { b: { c: [Object] } } }",
  );
  assertEquals(
    stripColor(inspectArgs([nestedObj], { depth: 4 })),
    "{ a: { b: { c: { d: [Object] } } } }",
  );
  assertEquals(stripColor(inspectArgs([nestedObj], { depth: 0 })), "[Object]");
  assertEquals(
    stripColor(inspectArgs([nestedObj])),
    "{ a: { b: { c: { d: [Object] } } } }",
  );
  // test inspect is working the same way
  assertEquals(
    stripColor(Deno.inspect(nestedObj, { depth: 4 })),
    "{ a: { b: { c: { d: [Object] } } } }",
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
}`,
  );
});

unitTest(function consoleTestStringifyIterable() {
  const shortArray = [1, 2, 3, 4, 5];
  assertEquals(stringify(shortArray), "[ 1, 2, 3, 4, 5 ]");

  const longArray = new Array(200).fill(0);
  assertEquals(
    stringify(longArray),
    `[
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  ... 100 more items
]`,
  );

  const obj = { a: "a", longArray };
  assertEquals(
    stringify(obj),
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
}`,
  );

  const shortMap = new Map([
    ["a", 0],
    ["b", 1],
  ]);
  assertEquals(stringify(shortMap), `Map { "a" => 0, "b" => 1 }`);

  const longMap = new Map();
  for (const key of Array(200).keys()) {
    longMap.set(`${key}`, key);
  }
  assertEquals(
    stringify(longMap),
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
}`,
  );

  const shortSet = new Set([1, 2, 3]);
  assertEquals(stringify(shortSet), `Set { 1, 2, 3 }`);
  const longSet = new Set();
  for (const key of Array(200).keys()) {
    longSet.add(key);
  }
  assertEquals(
    stringify(longSet),
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
}`,
  );

  const withEmptyEl = Array(10);
  withEmptyEl.fill(0, 4, 6);
  assertEquals(
    stringify(withEmptyEl),
    `[ <4 empty items>, 0, 0, <4 empty items> ]`,
  );

  /* TODO(ry) Fix this test
  const lWithEmptyEl = Array(200);
  lWithEmptyEl.fill(0, 50, 80);
  assertEquals(
    stringify(lWithEmptyEl),
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

unitTest(function consoleTestStringifyIterableWhenGrouped(): void {
  const withOddNumberOfEls = new Float64Array(
    [
      2.1,
      2.01,
      2.001,
      2.0001,
      2.00001,
      2.000001,
      2.0000001,
      2.00000001,
      2.000000001,
      2.0000000001,
      2,
    ],
  );
  assertEquals(
    stringify(withOddNumberOfEls),
    `Float64Array(11) [
          2.1,         2.01,
        2.001,       2.0001,
      2.00001,     2.000001,
    2.0000001,   2.00000001,
  2.000000001, 2.0000000001,
            2
]`,
  );
  const withEvenNumberOfEls = new Float64Array(
    [
      2.1,
      2.01,
      2.001,
      2.0001,
      2.00001,
      2.000001,
      2.0000001,
      2.00000001,
      2.000000001,
      2.0000000001,
      2,
      2,
    ],
  );
  assertEquals(
    stringify(withEvenNumberOfEls),
    `Float64Array(12) [
          2.1,         2.01,
        2.001,       2.0001,
      2.00001,     2.000001,
    2.0000001,   2.00000001,
  2.000000001, 2.0000000001,
            2,            2
]`,
  );
  const withThreeColumns = [
    2,
    2.1,
    2.11,
    2,
    2.111,
    2.1111,
    2,
    2.1,
    2.11,
    2,
    2.1,
  ];
  assertEquals(
    stringify(withThreeColumns),
    `[
  2,   2.1,   2.11,
  2, 2.111, 2.1111,
  2,   2.1,   2.11,
  2,   2.1
]`,
  );
});

unitTest(async function consoleTestStringifyPromises(): Promise<void> {
  const pendingPromise = new Promise((_res, _rej) => {});
  assertEquals(stringify(pendingPromise), "Promise { <pending> }");

  const resolvedPromise = new Promise((res, _rej) => {
    res("Resolved!");
  });
  assertEquals(stringify(resolvedPromise), `Promise { "Resolved!" }`);

  let rejectedPromise;
  try {
    rejectedPromise = new Promise((_, rej) => {
      rej(Error("Whoops"));
    });
    await rejectedPromise;
  } catch (err) {
    // pass
  }
  const strLines = stringify(rejectedPromise).split("\n");
  assertEquals(strLines[0], "Promise {");
  assertEquals(strLines[1], "  <rejected> Error: Whoops");
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
    [customInspect](): never {
      throw new Error("BOOM");
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
    "B { [Symbol(Deno.customInspect)]: [Function: [Deno.customInspect]] }",
  );
});

unitTest(function consoleTestWithCustomInspectFunction(): void {
  function a() {}
  Object.assign(a, {
    [customInspect]() {
      return "b";
    },
  });

  assertEquals(stringify(a), "b");
});

unitTest(function consoleTestWithIntegerFormatSpecifier(): void {
  assertEquals(stringify("%i"), "%i");
  assertEquals(stringify("%i", 42.0), "42");
  assertEquals(stringify("%i", 42), "42");
  assertEquals(stringify("%i", "42"), "NaN");
  assertEquals(stringify("%i", 1.5), "1");
  assertEquals(stringify("%i", -0.5), "0");
  assertEquals(stringify("%i", ""), "NaN");
  assertEquals(stringify("%i", Symbol()), "NaN");
  assertEquals(stringify("%i %d", 42, 43), "42 43");
  assertEquals(stringify("%d %i", 42), "42 %i");
  assertEquals(stringify("%d", 12345678901234567890123), "1");
  assertEquals(
    stringify("%i", 12345678901234567890123n),
    "12345678901234567890123n",
  );
});

unitTest(function consoleTestWithFloatFormatSpecifier(): void {
  assertEquals(stringify("%f"), "%f");
  assertEquals(stringify("%f", 42.0), "42");
  assertEquals(stringify("%f", 42), "42");
  assertEquals(stringify("%f", "42"), "NaN");
  assertEquals(stringify("%f", 1.5), "1.5");
  assertEquals(stringify("%f", -0.5), "-0.5");
  assertEquals(stringify("%f", Math.PI), "3.141592653589793");
  assertEquals(stringify("%f", ""), "NaN");
  assertEquals(stringify("%f", Symbol("foo")), "NaN");
  assertEquals(stringify("%f", 5n), "NaN");
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
  assertEquals(stringify("%o", "foo"), `"foo"`);
  assertEquals(stringify("o: %o, a: %O", {}, []), "o: {}, a: []");
  assertEquals(stringify("%o", { a: 42 }), "{ a: 42 }");
  assertEquals(
    stringify("%o", { a: { b: { c: { d: new Set([1]) } } } }),
    "{ a: { b: { c: { d: [Set] } } } }",
  );
});

unitTest(function consoleTestWithStyleSpecifier(): void {
  assertEquals(stringify("%cfoo%cbar"), "%cfoo%cbar");
  assertEquals(stringify("%cfoo%cbar", ""), "foo%cbar");
  assertEquals(stripColor(stringify("%cfoo%cbar", "", "color: red")), "foobar");
});

unitTest(function consoleParseCssColor(): void {
  assertEquals(parseCssColor("black"), [0, 0, 0]);
  assertEquals(parseCssColor("darkmagenta"), [139, 0, 139]);
  assertEquals(parseCssColor("slateblue"), [106, 90, 205]);
  assertEquals(parseCssColor("#ffaa00"), [255, 170, 0]);
  assertEquals(parseCssColor("#ffaa00"), [255, 170, 0]);
  assertEquals(parseCssColor("#18d"), [16, 128, 208]);
  assertEquals(parseCssColor("#18D"), [16, 128, 208]);
  assertEquals(parseCssColor("rgb(100, 200, 50)"), [100, 200, 50]);
  assertEquals(parseCssColor("rgb(+100.3, -200, .5)"), [100, 0, 1]);
  assertEquals(parseCssColor("hsl(75, 60%, 40%)"), [133, 163, 41]);

  assertEquals(parseCssColor("rgb(100,200,50)"), [100, 200, 50]);
  assertEquals(
    parseCssColor("rgb( \t\n100 \t\n, \t\n200 \t\n, \t\n50 \t\n)"),
    [100, 200, 50],
  );
});

unitTest(function consoleParseCss(): void {
  assertEquals(
    parseCss("background-color: red"),
    { ...DEFAULT_CSS, backgroundColor: [255, 0, 0] },
  );
  assertEquals(parseCss("color: blue"), { ...DEFAULT_CSS, color: [0, 0, 255] });
  assertEquals(
    parseCss("font-weight: bold"),
    { ...DEFAULT_CSS, fontWeight: "bold" },
  );
  assertEquals(
    parseCss("font-style: italic"),
    { ...DEFAULT_CSS, fontStyle: "italic" },
  );
  assertEquals(
    parseCss("font-style: oblique"),
    { ...DEFAULT_CSS, fontStyle: "italic" },
  );
  assertEquals(
    parseCss("text-decoration-color: green"),
    { ...DEFAULT_CSS, textDecorationColor: [0, 128, 0] },
  );
  assertEquals(
    parseCss("text-decoration-line: underline overline line-through"),
    {
      ...DEFAULT_CSS,
      textDecorationLine: ["underline", "overline", "line-through"],
    },
  );
  assertEquals(
    parseCss("text-decoration: yellow underline"),
    {
      ...DEFAULT_CSS,
      textDecorationColor: [255, 255, 0],
      textDecorationLine: ["underline"],
    },
  );

  assertEquals(
    parseCss("color:red;font-weight:bold;"),
    { ...DEFAULT_CSS, color: [255, 0, 0], fontWeight: "bold" },
  );
  assertEquals(
    parseCss(
      " \t\ncolor \t\n: \t\nred \t\n; \t\nfont-weight \t\n: \t\nbold \t\n; \t\n",
    ),
    { ...DEFAULT_CSS, color: [255, 0, 0], fontWeight: "bold" },
  );
  assertEquals(
    parseCss("color: red; font-weight: bold, font-style: italic"),
    { ...DEFAULT_CSS, color: [255, 0, 0] },
  );
});

unitTest(function consoleCssToAnsi(): void {
  assertEquals(
    cssToAnsiEsc({ ...DEFAULT_CSS, backgroundColor: [200, 201, 202] }),
    "_[48;2;200;201;202m",
  );
  assertEquals(
    cssToAnsiEsc({ ...DEFAULT_CSS, color: [203, 204, 205] }),
    "_[38;2;203;204;205m",
  );
  assertEquals(cssToAnsiEsc({ ...DEFAULT_CSS, fontWeight: "bold" }), "_[1m");
  assertEquals(cssToAnsiEsc({ ...DEFAULT_CSS, fontStyle: "italic" }), "_[3m");
  assertEquals(
    cssToAnsiEsc({ ...DEFAULT_CSS, textDecorationColor: [206, 207, 208] }),
    "_[58;2;206;207;208m",
  );
  assertEquals(
    cssToAnsiEsc({ ...DEFAULT_CSS, textDecorationLine: ["underline"] }),
    "_[4m",
  );
  assertEquals(
    cssToAnsiEsc(
      { ...DEFAULT_CSS, textDecorationLine: ["overline", "line-through"] },
    ),
    "_[9m_[53m",
  );
  assertEquals(
    cssToAnsiEsc(
      { ...DEFAULT_CSS, color: [203, 204, 205], fontWeight: "bold" },
    ),
    "_[38;2;203;204;205m_[1m",
  );
  assertEquals(
    cssToAnsiEsc(
      { ...DEFAULT_CSS, color: [0, 0, 0], fontWeight: "bold" },
      { ...DEFAULT_CSS, color: [203, 204, 205], fontStyle: "italic" },
    ),
    "_[38;2;0;0;0m_[1m_[23m",
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
        .includes("MyError: This is an error"),
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
  // deno-lint-ignore no-explicit-any
  csl: any,
  out: StringBuffer,
  err?: StringBuffer,
  both?: StringBuffer,
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
    },
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
`,
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
`,
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
`,
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
`,
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
`,
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
`,
    );
  });
  mockConsole((console, out): void => {
    console.table(
      new Map([
        [1, "one"],
        [2, "two"],
      ]),
    );
    assertEquals(
      stripColor(out.toString()),
      `┌────────────┬─────┬────────┐
│ (iter idx) │ Key │ Values │
├────────────┼─────┼────────┤
│     0      │  1  │ "one"  │
│     1      │  2  │ "two"  │
└────────────┴─────┴────────┘
`,
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
`,
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
`,
    );
  });
  mockConsole((console, out): void => {
    console.table([]);
    assertEquals(
      stripColor(out.toString()),
      `┌───────┐
│ (idx) │
├───────┤
└───────┘
`,
    );
  });
  mockConsole((console, out): void => {
    console.table({});
    assertEquals(
      stripColor(out.toString()),
      `┌───────┐
│ (idx) │
├───────┤
└───────┘
`,
    );
  });
  mockConsole((console, out): void => {
    console.table(new Set());
    assertEquals(
      stripColor(out.toString()),
      `┌────────────┐
│ (iter idx) │
├────────────┤
└────────────┘
`,
    );
  });
  mockConsole((console, out): void => {
    console.table(new Map());
    assertEquals(
      stripColor(out.toString()),
      `┌────────────┐
│ (iter idx) │
├────────────┤
└────────────┘
`,
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
`,
    );
  });
  mockConsole((console, out): void => {
    console.table([
      [1, 2],
      [3, 4],
    ]);
    assertEquals(
      stripColor(out.toString()),
      `┌───────┬───┬───┐
│ (idx) │ 0 │ 1 │
├───────┼───┼───┤
│   0   │ 1 │ 2 │
│   1   │ 3 │ 4 │
└───────┴───┴───┘
`,
    );
  });
  mockConsole((console, out): void => {
    console.table({ 1: { a: 4, b: 5 }, 2: null, 3: { b: 6, c: 7 } }, ["b"]);
    assertEquals(
      stripColor(out.toString()),
      `┌───────┬───┐
│ (idx) │ b │
├───────┼───┤
│   1   │ 5 │
│   2   │   │
│   3   │ 6 │
└───────┴───┘
`,
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
    assertEquals(stripColor(out.toString()), "Invalid Date\n");
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
    assertEquals(out.toString(), "    DIR\n");
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
    assertEquals(out.toString(), "    DIRXML\n");
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

unitTest(function inspectString(): void {
  assertEquals(
    stripColor(Deno.inspect("\0")),
    `"\\x00"`,
  );
  assertEquals(
    stripColor(Deno.inspect("\x1b[2J")),
    `"\\x1b[2J"`,
  );
});

unitTest(function inspectGetters(): void {
  assertEquals(
    stripColor(Deno.inspect({
      get foo() {
        return 0;
      },
    })),
    "{ foo: [Getter] }",
  );

  assertEquals(
    stripColor(Deno.inspect({
      get foo() {
        return 0;
      },
    }, { getters: true })),
    "{ foo: 0 }",
  );

  assertEquals(
    Deno.inspect({
      get foo() {
        throw new Error("bar");
      },
    }, { getters: true }),
    "{ foo: [Thrown Error: bar] }",
  );
});

unitTest(function inspectPrototype(): void {
  class A {}
  assertEquals(Deno.inspect(A.prototype), "A {}");
});

unitTest(function inspectSorted(): void {
  assertEquals(
    stripColor(Deno.inspect({ b: 2, a: 1 }, { sorted: true })),
    "{ a: 1, b: 2 }",
  );
  assertEquals(
    stripColor(Deno.inspect(new Set(["b", "a"]), { sorted: true })),
    `Set { "a", "b" }`,
  );
  assertEquals(
    stripColor(Deno.inspect(
      new Map([
        ["b", 2],
        ["a", 1],
      ]),
      { sorted: true },
    )),
    `Map { "a" => 1, "b" => 2 }`,
  );
});

unitTest(function inspectTrailingComma(): void {
  assertEquals(
    stripColor(Deno.inspect(
      [
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      ],
      { trailingComma: true },
    )),
    `[
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
]`,
  );
  assertEquals(
    stripColor(Deno.inspect(
      {
        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: 1,
        bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: 2,
      },
      { trailingComma: true },
    )),
    `{
  aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: 1,
  bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: 2,
}`,
  );
  assertEquals(
    stripColor(Deno.inspect(
      new Set([
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      ]),
      { trailingComma: true },
    )),
    `Set {
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
}`,
  );
  assertEquals(
    stripColor(Deno.inspect(
      new Map([
        ["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 1],
        ["bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", 2],
      ]),
      { trailingComma: true },
    )),
    `Map {
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" => 1,
  "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" => 2,
}`,
  );
});

unitTest(function inspectCompact(): void {
  assertEquals(
    stripColor(Deno.inspect({ a: 1, b: 2 }, { compact: false })),
    `{
  a: 1,
  b: 2
}`,
  );
});

unitTest(function inspectIterableLimit(): void {
  assertEquals(
    stripColor(Deno.inspect(["a", "b", "c"], { iterableLimit: 2 })),
    `[ "a", "b", ... 1 more items ]`,
  );
  assertEquals(
    stripColor(Deno.inspect(new Set(["a", "b", "c"]), { iterableLimit: 2 })),
    `Set { "a", "b", ... 1 more items }`,
  );
  assertEquals(
    stripColor(Deno.inspect(
      new Map([
        ["a", 1],
        ["b", 2],
        ["c", 3],
      ]),
      { iterableLimit: 2 },
    )),
    `Map { "a" => 1, "b" => 2, ... 1 more items }`,
  );
});

unitTest(function inspectProxy(): void {
  assertEquals(
    stripColor(Deno.inspect(
      new Proxy([1, 2, 3], { get(): void {} }),
    )),
    "[ 1, 2, 3 ]",
  );
  assertEquals(
    stripColor(Deno.inspect(
      new Proxy({ key: "value" }, { get(): void {} }),
    )),
    `{ key: "value" }`,
  );
  assertEquals(
    stripColor(Deno.inspect(
      new Proxy([1, 2, 3], { get(): void {} }),
      { showProxy: true },
    )),
    "Proxy [ [ 1, 2, 3 ], { get: [Function: get] } ]",
  );
  assertEquals(
    stripColor(Deno.inspect(
      new Proxy({ a: 1 }, {
        set(): boolean {
          return false;
        },
      }),
      { showProxy: true },
    )),
    "Proxy [ { a: 1 }, { set: [Function: set] } ]",
  );
  assertEquals(
    stripColor(Deno.inspect(
      new Proxy([1, 2, 3, 4, 5, 6, 7], { get(): void {} }),
      { showProxy: true },
    )),
    `Proxy [ [
    1, 2, 3, 4,
    5, 6, 7
  ], { get: [Function: get] } ]`,
  );
  assertEquals(
    stripColor(Deno.inspect(
      new Proxy(function fn() {}, { get(): void {} }),
      { showProxy: true },
    )),
    "Proxy [ [Function: fn], { get: [Function: get] } ]",
  );
});

unitTest(function inspectColors(): void {
  assertEquals(Deno.inspect(1), "1");
  assertStringIncludes(Deno.inspect(1, { colors: true }), "\x1b[");
});
