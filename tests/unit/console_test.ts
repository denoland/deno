// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(ry) The unit test functions in this module are too coarse. They should
// be broken up into smaller bits.

// TODO(ry) These tests currently strip all the ANSI colors out. We don't have a
// good way to control whether we produce color output or not since
// std/fmt/colors auto determines whether to put colors in or not. We need
// better infrastructure here so we can properly test the colors.

import {
  assert,
  assertEquals,
  assertStringIncludes,
  assertThrows,
} from "./test_util.ts";
import { stripAnsiCode } from "@std/fmt/colors";

const customInspect = Symbol.for("Deno.customInspect");
const {
  Console,
  cssToAnsi: cssToAnsi_,
  inspectArgs,
  parseCss: parseCss_,
  parseCssColor: parseCssColor_,
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
} = Deno[Deno.internal];

function stringify(...args: unknown[]): string {
  return stripAnsiCode(inspectArgs(args).replace(/\n$/, ""));
}

interface Css {
  backgroundColor: [number, number, number] | string | null;
  color: [number, number, number] | string | null;
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

function parseCssColor(colorString: string): [number, number, number] | null {
  return parseCssColor_(colorString);
}

/** ANSI-fy the CSS, replace "\x1b" with "_". */
function cssToAnsiEsc(css: Css, prevCss: Css | null = null): string {
  return cssToAnsi_(css, prevCss).replaceAll("\x1b", "_");
}

// test cases from web-platform-tests
// via https://github.com/web-platform-tests/wpt/blob/master/console/console-is-a-namespace.any.js
Deno.test(function consoleShouldBeANamespace() {
  const prototype1 = Object.getPrototypeOf(console);
  const prototype2 = Object.getPrototypeOf(prototype1);

  assertEquals(Object.getOwnPropertyNames(prototype1).length, 0);
  assertEquals(prototype2, Object.prototype);
});

Deno.test(function consoleHasRightInstance() {
  assert(console instanceof Console);
  assertEquals({} instanceof Console, false);
});

Deno.test(function consoleTestAssertShouldNotThrowError() {
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

Deno.test(function consoleTestStringifyComplexObjects() {
  assertEquals(stringify("foo"), "foo");
  assertEquals(stringify(["foo", "bar"]), `[ "foo", "bar" ]`);
  assertEquals(stringify({ foo: "bar" }), `{ foo: "bar" }`);
});

Deno.test(
  function consoleTestStringifyComplexObjectsWithEscapedSequences() {
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
  [Symbol("foo\\b")]: 'Symbol("foo\\n")',
  [Symbol("bar\\n")]: 'Symbol("bar\\n")',
  [Symbol("bar\\r")]: 'Symbol("bar\\r")',
  [Symbol("baz\\t")]: 'Symbol("baz\\t")',
  [Symbol("qux\\x00")]: 'Symbol("qux\\x00")'
}`,
    );
    assertEquals(
      stringify(new Set(["foo\n", "foo\r", "foo\0"])),
      `Set(3) { "foo\\n", "foo\\r", "foo\\x00" }`,
    );
  },
);

Deno.test(function consoleTestStringifyQuotes() {
  assertEquals(stringify(["\\"]), `[ "\\\\" ]`);
  assertEquals(stringify(['\\,"']), `[ '\\\\,"' ]`);
  assertEquals(stringify([`\\,",'`]), `[ \`\\\\,",'\` ]`);
  assertEquals(stringify(["\\,\",',`"]), `[ "\\\\,\\",',\`" ]`);
});

Deno.test(function consoleTestStringifyLongStrings() {
  const veryLongString = "a".repeat(10_100);
  // If we stringify an object containing the long string, it gets abbreviated.
  let actual = stringify({ veryLongString });
  assert(actual.includes("..."));
  assert(actual.length < 10_100);
  // However if we stringify the string itself, we get it exactly.
  actual = stringify(veryLongString);
  assertEquals(actual, veryLongString);
});

Deno.test(function consoleTestStringifyCircular() {
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
  const nestedObjExpected = `<ref *1> {
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
  nFunc: [Function: anonymous],
  extendedCstr: [class Extended extends Base],
  o: {
    num: 2,
    bool: false,
    str: "b",
    method: [Function: method],
    un: undefined,
    nu: null,
    nested: [Circular *1],
    emptyObj: {},
    arr: [ 1, "s", false, null, [Circular *1] ],
    baseClass: Base { a: 1 }
  }
}`;

  assertEquals(stringify(1), "1");
  assertEquals(stringify(-0), "-0");
  assertEquals(stringify(1n), "1n");
  assertEquals(stringify("s"), "s");
  assertEquals(stringify(false), "false");
  assertEquals(stringify(new Number(1)), "[Number: 1]");
  assertEquals(stringify(new Number(-0)), "[Number: -0]");
  assertEquals(stringify(Object(1n)), "[BigInt: 1n]");
  assertEquals(stringify(new Boolean(true)), "[Boolean: true]");
  assertEquals(stringify(new String("deno")), `[String: "deno"]`);
  assertEquals(stringify(/[0-9]*/), "/[0-9]*/");
  assertEquals(
    stringify(new Date("2018-12-10T02:26:59.002Z")),
    "2018-12-10T02:26:59.002Z",
  );
  assertEquals(stringify(new Set([1, 2, 3])), "Set(3) { 1, 2, 3 }");
  assertEquals(
    stringify(new Set([1, 2, 3]).values()),
    "[Set Iterator] { 1, 2, 3 }",
  );
  assertEquals(
    stringify(new Set([1, 2, 3]).entries()),
    "[Set Entries] { [ 1, 1 ], [ 2, 2 ], [ 3, 3 ] }",
  );
  assertEquals(
    stringify(
      new Map([
        [1, "one"],
        [2, "two"],
      ]),
    ),
    `Map(2) { 1 => "one", 2 => "two" }`,
  );
  assertEquals(
    stringify(new Map([[1, "one"], [2, "two"]]).values()),
    `[Map Iterator] { "one", "two" }`,
  );
  assertEquals(
    stringify(new Map([[1, "one"], [2, "two"]]).entries()),
    `[Map Entries] { [ 1, "one" ], [ 2, "two" ] }`,
  );
  assertEquals(stringify(new WeakSet()), "WeakSet { <items unknown> }");
  assertEquals(stringify(new WeakMap()), "WeakMap { <items unknown> }");
  assertEquals(stringify(Symbol(1)), `Symbol("1")`);
  assertEquals(stringify(Object(Symbol(1))), `[Symbol: Symbol("1")]`);
  assertEquals(stringify(null), "null");
  assertEquals(stringify(undefined), "undefined");
  assertEquals(stringify(new Extended()), "Extended { a: 1, b: 2 }");
  assertEquals(
    stringify(function f() {}),
    "[Function: f]",
  );
  assertEquals(
    stringify(async function af() {}),
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
  assertEquals(stringify(Uint8Array.prototype), "TypedArray {}");
  assertEquals(
    stringify({ a: { b: { c: { d: new Set([1]) } } } }),
    `{
  a: {
    b: { c: { d: Set(1) { 1 } } }
  }
}`,
  );
  assertEquals(stringify(nestedObj), nestedObjExpected);
  assertEquals(
    stringify(JSON),
    "Object [JSON] {}",
  );
  assertEquals(
    stringify(new Console(() => {})),
    `Object [console] {
  log: [Function: log],
  debug: [Function: debug],
  info: [Function: info],
  dir: [Function: dir],
  dirxml: [Function: dir],
  warn: [Function: warn],
  error: [Function: error],
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
  profile: [Function: profile],
  profileEnd: [Function: profileEnd],
  timeStamp: [Function: timeStamp],
  indentLevel: 0,
  [Symbol(isConsoleInstance)]: true
}`,
  );
  assertEquals(
    stringify({ str: 1, [Symbol.for("sym")]: 2, [Symbol.toStringTag]: "TAG" }),
    `Object [TAG] {
  str: 1,
  [Symbol(sym)]: 2,
  [Symbol(Symbol.toStringTag)]: "TAG"
}`,
  );
  // test inspect is working the same
  assertEquals(stripAnsiCode(Deno.inspect(nestedObj)), nestedObjExpected);
});

Deno.test(function consoleTestStringifyMultipleCircular() {
  const y = { a: { b: {} }, foo: { bar: {} } };
  y.a.b = y.a;
  y.foo.bar = y.foo;
  assertEquals(
    stringify(y),
    "{\n" +
      "  a: <ref *1> { b: [Circular *1] },\n" +
      "  foo: <ref *2> { bar: [Circular *2] }\n" +
      "}",
  );
});

Deno.test(function consoleTestStringifyFunctionWithPrototypeRemoved() {
  const f = function f() {};
  Reflect.setPrototypeOf(f, null);
  assertEquals(stringify(f), "[Function (null prototype): f]");
  const af = async function af() {};
  Reflect.setPrototypeOf(af, null);
  assertEquals(stringify(af), "[AsyncFunction (null prototype): af]");
  const gf = function* gf() {};
  Reflect.setPrototypeOf(gf, null);
  assertEquals(stringify(gf), "[GeneratorFunction (null prototype): gf]");
  const agf = async function* agf() {};
  Reflect.setPrototypeOf(agf, null);
  assertEquals(
    stringify(agf),
    "[AsyncGeneratorFunction (null prototype): agf]",
  );
});

Deno.test(function consoleTestStringifyFunctionWithProperties() {
  const f = () => "test";
  f.x = () => "foo";
  f.y = 3;
  f.z = () => "baz";
  f.b = function bar() {};
  f.a = new Map();
  assertEquals(
    stringify({ f }),
    `{
  f: [Function: f] {
    x: [Function (anonymous)],
    y: 3,
    z: [Function (anonymous)],
    b: [Function: bar],
    a: Map(0) {}
  }
}`,
  );

  const t = () => {};
  t.x = f;
  f.s = f;
  f.t = t;
  assertEquals(
    stringify({ f }),
    `{
  f: <ref *1> [Function: f] {
    x: [Function (anonymous)],
    y: 3,
    z: [Function (anonymous)],
    b: [Function: bar],
    a: Map(0) {},
    s: [Circular *1],
    t: [Function: t] { x: [Circular *1] }
  }
}`,
  );

  assertEquals(
    stringify(Array),
    `[Function: Array]`,
  );

  assertEquals(
    stripAnsiCode(Deno.inspect(Array, { showHidden: true })),
    `<ref *1> [Function: Array] {
  [length]: 1,
  [name]: "Array",
  [prototype]: Object(0) [
    [length]: 0,
    [constructor]: [Circular *1],
    [at]: [Function: at] { [length]: 1, [name]: "at" },
    [concat]: [Function: concat] { [length]: 1, [name]: "concat" },
    [copyWithin]: [Function: copyWithin] { [length]: 2, [name]: "copyWithin" },
    [fill]: [Function: fill] { [length]: 1, [name]: "fill" },
    [find]: [Function: find] { [length]: 1, [name]: "find" },
    [findIndex]: [Function: findIndex] { [length]: 1, [name]: "findIndex" },
    [findLast]: [Function: findLast] { [length]: 1, [name]: "findLast" },
    [findLastIndex]: [Function: findLastIndex] { [length]: 1, [name]: "findLastIndex" },
    [lastIndexOf]: [Function: lastIndexOf] { [length]: 1, [name]: "lastIndexOf" },
    [pop]: [Function: pop] { [length]: 0, [name]: "pop" },
    [push]: [Function: push] { [length]: 1, [name]: "push" },
    [reverse]: [Function: reverse] { [length]: 0, [name]: "reverse" },
    [shift]: [Function: shift] { [length]: 0, [name]: "shift" },
    [unshift]: [Function: unshift] { [length]: 1, [name]: "unshift" },
    [slice]: [Function: slice] { [length]: 2, [name]: "slice" },
    [sort]: [Function: sort] { [length]: 1, [name]: "sort" },
    [splice]: [Function: splice] { [length]: 2, [name]: "splice" },
    [includes]: [Function: includes] { [length]: 1, [name]: "includes" },
    [indexOf]: [Function: indexOf] { [length]: 1, [name]: "indexOf" },
    [join]: [Function: join] { [length]: 1, [name]: "join" },
    [keys]: [Function: keys] { [length]: 0, [name]: "keys" },
    [entries]: [Function: entries] { [length]: 0, [name]: "entries" },
    [values]: [Function: values] { [length]: 0, [name]: "values" },
    [forEach]: [Function: forEach] { [length]: 1, [name]: "forEach" },
    [filter]: [Function: filter] { [length]: 1, [name]: "filter" },
    [flat]: [Function: flat] { [length]: 0, [name]: "flat" },
    [flatMap]: [Function: flatMap] { [length]: 1, [name]: "flatMap" },
    [map]: [Function: map] { [length]: 1, [name]: "map" },
    [every]: [Function: every] { [length]: 1, [name]: "every" },
    [some]: [Function: some] { [length]: 1, [name]: "some" },
    [reduce]: [Function: reduce] { [length]: 1, [name]: "reduce" },
    [reduceRight]: [Function: reduceRight] { [length]: 1, [name]: "reduceRight" },
    [toReversed]: [Function: toReversed] { [length]: 0, [name]: "toReversed" },
    [toSorted]: [Function: toSorted] { [length]: 1, [name]: "toSorted" },
    [toSpliced]: [Function: toSpliced] { [length]: 2, [name]: "toSpliced" },
    [with]: [Function: with] { [length]: 2, [name]: "with" },
    [toLocaleString]: [Function: toLocaleString] { [length]: 0, [name]: "toLocaleString" },
    [toString]: [Function: toString] { [length]: 0, [name]: "toString" },
    [Symbol(Symbol.iterator)]: [Function: values] { [length]: 0, [name]: "values" },
    [Symbol(Symbol.unscopables)]: [Object: null prototype] {
      at: true,
      copyWithin: true,
      entries: true,
      fill: true,
      find: true,
      findIndex: true,
      findLast: true,
      findLastIndex: true,
      flat: true,
      flatMap: true,
      includes: true,
      keys: true,
      toReversed: true,
      toSorted: true,
      toSpliced: true,
      values: true
    }
  ],
  [isArray]: [Function: isArray] { [length]: 1, [name]: "isArray" },
  [from]: [Function: from] { [length]: 1, [name]: "from" },
  [fromAsync]: [Function: fromAsync] { [length]: 1, [name]: "fromAsync" },
  [of]: [Function: of] { [length]: 0, [name]: "of" },
  [Symbol(Symbol.species)]: [Getter]
}`,
  );
});

Deno.test(function consoleTestStringifyWithDepth() {
  // deno-lint-ignore no-explicit-any
  const nestedObj: any = { a: { b: { c: { d: { e: { f: 42 } } } } } };
  assertEquals(
    stripAnsiCode(inspectArgs([nestedObj], { depth: 3 })),
    "{\n  a: { b: { c: { d: [Object] } } }\n}",
  );
  assertEquals(
    stripAnsiCode(inspectArgs([nestedObj], { depth: 4 })),
    "{\n  a: {\n    b: { c: { d: { e: [Object] } } }\n  }\n}",
  );
  assertEquals(
    stripAnsiCode(inspectArgs([nestedObj], { depth: 0 })),
    "{ a: [Object] }",
  );
  assertEquals(
    stripAnsiCode(inspectArgs([nestedObj])),
    "{\n  a: {\n    b: { c: { d: { e: [Object] } } }\n  }\n}",
  );
  // test inspect is working the same way
  assertEquals(
    stripAnsiCode(Deno.inspect(nestedObj, { depth: 4 })),
    "{\n  a: {\n    b: { c: { d: { e: [Object] } } }\n  }\n}",
  );
});

Deno.test(function consoleTestStringifyLargeObject() {
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

Deno.test(function consoleTestStringifyIterable() {
  const shortArray = [1, 2, 3, 4, 5];
  assertEquals(stringify(shortArray), "[ 1, 2, 3, 4, 5 ]");

  const longArray = new Array(200).fill(0);
  assertEquals(
    stringify(longArray),
    `[
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0,
  ... 100 more items
]`,
  );

  const obj = { a: "a", longArray };
  assertEquals(
    stringify(obj),
    `{
  a: "a",
  longArray: [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0,
    ... 100 more items
  ]
}`,
  );

  const shortMap = new Map([
    ["a", 0],
    ["b", 1],
  ]);
  assertEquals(stringify(shortMap), `Map(2) { "a" => 0, "b" => 1 }`);

  const longMap = new Map();
  for (const key of Array(200).keys()) {
    longMap.set(`${key}`, key);
  }
  assertEquals(
    stringify(longMap),
    `Map(200) {
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
  assertEquals(stringify(shortSet), `Set(3) { 1, 2, 3 }`);
  const longSet = new Set();
  for (const key of Array(200).keys()) {
    longSet.add(key);
  }
  assertEquals(
    stringify(longSet),
    `Set(200) {
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

  const emptyArray = Array(5000);
  assertEquals(
    stringify(emptyArray),
    `[ <5000 empty items> ]`,
  );

  assertEquals(
    stringify(Array(1)),
    `[ <1 empty item> ]`,
  );

  assertEquals(
    stringify([, , 1]),
    `[ <2 empty items>, 1 ]`,
  );

  assertEquals(
    stringify([1, , , 1]),
    `[ 1, <2 empty items>, 1 ]`,
  );

  const withEmptyElAndMoreItems = Array(500);
  withEmptyElAndMoreItems.fill(0, 50, 80);
  withEmptyElAndMoreItems.fill(2, 100, 120);
  withEmptyElAndMoreItems.fill(3, 140, 160);
  withEmptyElAndMoreItems.fill(4, 180);
  assertEquals(
    stringify(withEmptyElAndMoreItems),
    `[
  <50 empty items>, 0,                0, 0,
  0,                0,                0, 0,
  0,                0,                0, 0,
  0,                0,                0, 0,
  0,                0,                0, 0,
  0,                0,                0, 0,
  0,                0,                0, 0,
  0,                0,                0, <20 empty items>,
  2,                2,                2, 2,
  2,                2,                2, 2,
  2,                2,                2, 2,
  2,                2,                2, 2,
  2,                2,                2, 2,
  <20 empty items>, 3,                3, 3,
  3,                3,                3, 3,
  3,                3,                3, 3,
  3,                3,                3, 3,
  3,                3,                3, 3,
  3,                <20 empty items>, 4, 4,
  4,                4,                4, 4,
  4,                4,                4, 4,
  4,                4,                4, 4,
  4,                4,                4, 4,
  4,                4,                4, 4,
  4,                4,                4, 4,
  ... 294 more items
]`,
  );

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
]`,
  );
});

Deno.test(function consoleTestStringifyIterableWhenGrouped() {
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

Deno.test(function consoleTestIteratorValueAreNotConsumed() {
  const setIterator = new Set([1, 2, 3]).values();
  assertEquals(
    stringify(setIterator),
    "[Set Iterator] { 1, 2, 3 }",
  );
  assertEquals([...setIterator], [1, 2, 3]);
});

Deno.test(function consoleTestWeakSetAndWeakMapWithShowHidden() {
  assertEquals(
    stripAnsiCode(Deno.inspect(new WeakSet([{}]), { showHidden: true })),
    "WeakSet { {} }",
  );
  assertEquals(
    stripAnsiCode(
      Deno.inspect(new WeakMap([[{}, "foo"]]), { showHidden: true }),
    ),
    'WeakMap { {} => "foo" }',
  );
});

Deno.test(async function consoleTestStringifyPromises() {
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
  } catch (_err) {
    // pass
  }
  const strLines = stringify(rejectedPromise).split("\n");
  assertEquals(strLines[0], "Promise {");
  assertEquals(strLines[1], "  <rejected> Error: Whoops");
});

Deno.test(function consoleTestWithCustomInspector() {
  class A {
    [customInspect](
      inspect: unknown,
      options: Deno.InspectOptions,
    ): string {
      assertEquals(typeof inspect, "function");
      assertEquals(typeof options, "object");
      return "b";
    }
  }

  assertEquals(stringify(new A()), "b");
});

Deno.test(function consoleTestWithCustomInspectorUsingInspectFunc() {
  class A {
    [customInspect](
      inspect: (v: unknown, opts?: Deno.InspectOptions) => string,
    ): string {
      return "b " + inspect({ c: 1 });
    }
  }

  assertEquals(stringify(new A()), "b { c: 1 }");
});

Deno.test(function consoleTestWithConstructorError() {
  const obj = new Proxy({}, {
    getOwnPropertyDescriptor(_target, name) {
      if (name == "constructor") {
        throw "yikes";
      }
      return undefined;
    },
  });
  assertEquals(Deno.inspect(obj), "{}");
});

Deno.test(function consoleTestWithCustomInspectorError() {
  class A {
    [customInspect](): never {
      throw new Error("BOOM");
    }
  }

  const a = new A();
  assertThrows(
    () => stringify(a),
    Error,
    "BOOM",
    "Custom inspect won't attempt to parse if user defined function throws",
  );
  assertThrows(
    () => stringify(a),
    Error,
    "BOOM",
    "Inspect should fail and maintain a clear CTX_STACK",
  );
});

Deno.test(function consoleTestWithCustomInspectFunction() {
  function a() {}
  Object.assign(a, {
    [customInspect]() {
      return "b";
    },
  });

  assertEquals(stringify(a), "b");
});

Deno.test(function consoleTestWithIntegerFormatSpecifier() {
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

Deno.test(function consoleTestWithFloatFormatSpecifier() {
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

Deno.test(function consoleTestWithStringFormatSpecifier() {
  assertEquals(stringify("%s"), "%s");
  assertEquals(stringify("%s", undefined), "undefined");
  assertEquals(stringify("%s", "foo"), "foo");
  assertEquals(stringify("%s", 42), "42");
  assertEquals(stringify("%s", "42"), "42");
  assertEquals(stringify("%s %s", 42, 43), "42 43");
  assertEquals(stringify("%s %s", 42), "42 %s");
  assertEquals(stringify("%s", Symbol("foo")), "Symbol(foo)");
});

Deno.test(function consoleTestWithObjectFormatSpecifier() {
  assertEquals(stringify("%o"), "%o");
  assertEquals(stringify("%o", 42), "42");
  assertEquals(stringify("%o", "foo"), `"foo"`);
  assertEquals(stringify("o: %o, a: %O", {}, []), "o: {}, a: []");
  assertEquals(stringify("%o", { a: 42 }), "{ a: 42 }");
  assertEquals(
    stringify("%o", { a: { b: { c: { d: new Set([1]) } } } }),
    "{\n  a: {\n    b: { c: { d: Set(1) { 1 } } }\n  }\n}",
  );
});

Deno.test(function consoleTestWithStyleSpecifier() {
  assertEquals(stringify("%cfoo%cbar"), "%cfoo%cbar");
  assertEquals(stringify("%cfoo%cbar", ""), "foo%cbar");
  assertEquals(
    stripAnsiCode(stringify("%cfoo%cbar", "", "color: red")),
    "foobar",
  );
});

Deno.test(function consoleParseCssColor() {
  assertEquals(parseCssColor("inherit"), null);
  assertEquals(parseCssColor("black"), [0, 0, 0]);
  assertEquals(parseCssColor("darkmagenta"), [139, 0, 139]);
  assertEquals(parseCssColor("slateblue"), [106, 90, 205]);
  assertEquals(parseCssColor("#ffaa00"), [255, 170, 0]);
  assertEquals(parseCssColor("#ffAA00"), [255, 170, 0]);
  assertEquals(parseCssColor("#fa0"), [255, 170, 0]);
  assertEquals(parseCssColor("#FA0"), [255, 170, 0]);
  assertEquals(parseCssColor("#18d"), [17, 136, 221]);
  assertEquals(parseCssColor("#18D"), [17, 136, 221]);
  assertEquals(parseCssColor("#1188DD"), [17, 136, 221]);
  assertEquals(parseCssColor("rgb(100, 200, 50)"), [100, 200, 50]);
  assertEquals(parseCssColor("rgb(+100.3, -200, .5)"), [100, 0, 1]);
  assertEquals(parseCssColor("hsl(75, 60%, 40%)"), [133, 163, 41]);

  assertEquals(parseCssColor("rgb(100,200,50)"), [100, 200, 50]);
  assertEquals(
    parseCssColor("rgb( \t\n100 \t\n, \t\n200 \t\n, \t\n50 \t\n)"),
    [100, 200, 50],
  );
});

Deno.test(function consoleParseCss() {
  assertEquals(
    parseCss("background-color: inherit"),
    { ...DEFAULT_CSS, backgroundColor: "inherit" },
  );
  assertEquals(
    parseCss("color: inherit"),
    { ...DEFAULT_CSS, color: "inherit" },
  );
  assertEquals(
    parseCss("background-color: red"),
    { ...DEFAULT_CSS, backgroundColor: "red" },
  );
  assertEquals(parseCss("color: blue"), { ...DEFAULT_CSS, color: "blue" });
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
    { ...DEFAULT_CSS, color: "red", fontWeight: "bold" },
  );
  assertEquals(
    parseCss(
      " \t\ncolor \t\n: \t\nred \t\n; \t\nfont-weight \t\n: \t\nbold \t\n; \t\n",
    ),
    { ...DEFAULT_CSS, color: "red", fontWeight: "bold" },
  );
  assertEquals(
    parseCss("color: red; font-weight: bold, font-style: italic"),
    { ...DEFAULT_CSS, color: "red" },
  );
});

Deno.test(function consoleCssToAnsi() {
  assertEquals(
    cssToAnsiEsc({ ...DEFAULT_CSS, backgroundColor: "inherit" }),
    "_[49m",
  );
  assertEquals(
    cssToAnsiEsc({ ...DEFAULT_CSS, backgroundColor: "foo" }),
    "_[49m",
  );
  assertEquals(
    cssToAnsiEsc({ ...DEFAULT_CSS, backgroundColor: "black" }),
    "_[40m",
  );
  assertEquals(
    cssToAnsiEsc({ ...DEFAULT_CSS, color: "inherit" }),
    "_[39m",
  );
  assertEquals(
    cssToAnsiEsc({ ...DEFAULT_CSS, color: "blue" }),
    "_[34m",
  );
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

Deno.test(function consoleTestWithVariousOrInvalidFormatSpecifier() {
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

Deno.test(function consoleTestCallToStringOnLabel() {
  const methods = ["count", "countReset", "time", "timeLog", "timeEnd"];
  mockConsole((console) => {
    for (const method of methods) {
      let hasCalled = false;
      console[method]({
        toString() {
          hasCalled = true;
        },
      });
      assertEquals(hasCalled, true);
    }
  });
});

Deno.test(function consoleTestError() {
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

Deno.test(function consoleTestClear() {
  mockConsole((console, out) => {
    console.clear();
    assertEquals(out.toString(), "\x1b[1;1H" + "\x1b[0J");
  });
});

// Test bound this issue
Deno.test(function consoleDetachedLog() {
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
  add(x: string) {
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

function mockConsole(f: ConsoleExamineFunc) {
  const out = new StringBuffer();
  const err = new StringBuffer();
  const both = new StringBuffer();
  const csl = new Console(
    (x: string, level: number, printsNewLine: boolean) => {
      const content = x + (printsNewLine ? "\n" : "");
      const buf = level > 1 ? err : out;
      buf.add(content);
      both.add(content);
    },
  );
  f(csl, out, err, both);
}

// console.group test
Deno.test(function consoleGroup() {
  mockConsole((console, out) => {
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
Deno.test(function consoleGroupWarn() {
  mockConsole((console, _out, _err, both) => {
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
Deno.test(function consoleTable() {
  mockConsole((console, out) => {
    console.table({ a: "test", b: 1 });
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┬────────┐
│ (idx) │ Values │
├───────┼────────┤
│ a     │ "test" │
│ b     │ 1      │
└───────┴────────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table({ a: { b: 10 }, b: { b: 20, c: 30 } }, ["c"]);
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┬────┐
│ (idx) │ c  │
├───────┼────┤
│ a     │    │
│ b     │ 30 │
└───────┴────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table([[1, 1], [234, 2.34], [56789, 56.789]]);
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┬───────┬────────┐
│ (idx) │ 0     │ 1      │
├───────┼───────┼────────┤
│     0 │     1 │ 1      │
│     1 │   234 │ 2.34   │
│     2 │ 56789 │ 56.789 │
└───────┴───────┴────────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table([1, 2, [3, [4]], [5, 6], [[7], [8]]]);
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┬───────┬───────┬────────┐
│ (idx) │ 0     │ 1     │ Values │
├───────┼───────┼───────┼────────┤
│     0 │       │       │      1 │
│     1 │       │       │      2 │
│     2 │ 3     │ [ 4 ] │        │
│     3 │ 5     │ 6     │        │
│     4 │ [ 7 ] │ [ 8 ] │        │
└───────┴───────┴───────┴────────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table(new Set([1, 2, 3, "test"]));
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌────────────┬────────┐
│ (iter idx) │ Values │
├────────────┼────────┤
│          0 │ 1      │
│          1 │ 2      │
│          2 │ 3      │
│          3 │ "test" │
└────────────┴────────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table(
      new Map([
        [1, "one"],
        [2, "two"],
      ]),
    );
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌────────────┬─────┬────────┐
│ (iter idx) │ Key │ Values │
├────────────┼─────┼────────┤
│          0 │   1 │ "one"  │
│          1 │   2 │ "two"  │
└────────────┴─────┴────────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table({
      a: true,
      b: { c: { d: 10 }, e: [1, 2, [5, 6]] },
      f: "test",
      g: new Set([1, 2, 3, "test"]),
      h: new Map([[1, "one"]]),
    });
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┬───────────┬────────────────────┬────────┐
│ (idx) │ c         │ e                  │ Values │
├───────┼───────────┼────────────────────┼────────┤
│ a     │           │                    │ true   │
│ b     │ { d: 10 } │ [ 1, 2, [ 5, 6 ] ] │        │
│ f     │           │                    │ "test" │
│ g     │           │                    │        │
│ h     │           │                    │        │
└───────┴───────────┴────────────────────┴────────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table([
      1,
      "test",
      false,
      { a: 10 },
      ["test", { b: 20, c: "test" }],
    ]);
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┬────────┬──────────────────────┬────┬────────┐
│ (idx) │ 0      │ 1                    │ a  │ Values │
├───────┼────────┼──────────────────────┼────┼────────┤
│     0 │        │                      │    │ 1      │
│     1 │        │                      │    │ "test" │
│     2 │        │                      │    │ false  │
│     3 │        │                      │ 10 │        │
│     4 │ "test" │ { b: 20, c: "test" } │    │        │
└───────┴────────┴──────────────────────┴────┴────────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table([]);
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┐
│ (idx) │
├───────┤
└───────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table({});
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┐
│ (idx) │
├───────┤
└───────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table(new Set());
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌────────────┐
│ (iter idx) │
├────────────┤
└────────────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table(new Map());
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌────────────┐
│ (iter idx) │
├────────────┤
└────────────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table("test");
    assertEquals(out.toString(), "test\n");
  });
  mockConsole((console, out) => {
    console.table(["Hello", "你好", "Amapá"]);
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┬─────────┐
│ (idx) │ Values  │
├───────┼─────────┤
│     0 │ "Hello" │
│     1 │ "你好"  │
│     2 │ "Amapá" │
└───────┴─────────┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table([
      [1, 2],
      [3, 4],
    ]);
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┬───┬───┐
│ (idx) │ 0 │ 1 │
├───────┼───┼───┤
│     0 │ 1 │ 2 │
│     1 │ 3 │ 4 │
└───────┴───┴───┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table({ 1: { a: 4, b: 5 }, 2: null, 3: { b: 6, c: 7 } }, ["b"]);
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┬───┐
│ (idx) │ b │
├───────┼───┤
│     1 │ 5 │
│     2 │   │
│     3 │ 6 │
└───────┴───┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table([{ a: 0 }, { a: 1, b: 1 }, { a: 2 }, { a: 3, b: 3 }]);
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┬───┬───┐
│ (idx) │ a │ b │
├───────┼───┼───┤
│     0 │ 0 │   │
│     1 │ 1 │ 1 │
│     2 │ 2 │   │
│     3 │ 3 │ 3 │
└───────┴───┴───┘
`,
    );
  });
  mockConsole((console, out) => {
    console.table(
      [{ a: 0 }, { a: 1, c: 1 }, { a: 2 }, { a: 3, c: 3 }],
      ["a", "b", "c"],
    );
    assertEquals(
      stripAnsiCode(out.toString()),
      `\
┌───────┬───┬───┬───┐
│ (idx) │ a │ b │ c │
├───────┼───┼───┼───┤
│     0 │ 0 │   │   │
│     1 │ 1 │   │ 1 │
│     2 │ 2 │   │   │
│     3 │ 3 │   │ 3 │
└───────┴───┴───┴───┘
`,
    );
  });
});

// console.log(Error) test
Deno.test(function consoleLogShouldNotThrowError() {
  mockConsole((console) => {
    let result = 0;
    try {
      console.log(new Error("foo"));
      result = 1;
    } catch (_e) {
      result = 2;
    }
    assertEquals(result, 1);
  });

  // output errors to the console should not include "Uncaught"
  mockConsole((console, out) => {
    console.log(new Error("foo"));
    assertEquals(out.toString().includes("Uncaught"), false);
  });
});

Deno.test(function consoleLogShouldNotThrowErrorWhenInvalidCssColorsAreGiven() {
  mockConsole((console, out) => {
    console.log("%cfoo", "color: foo; background-color: bar;");
    assertEquals(stripAnsiCode(out.toString()), "foo\n");
  });
});

// console.log(Invalid Date) test
Deno.test(function consoleLogShouldNotThrowErrorWhenInvalidDateIsPassed() {
  mockConsole((console, out) => {
    const invalidDate = new Date("test");
    console.log(invalidDate);
    assertEquals(stripAnsiCode(out.toString()), "Invalid Date\n");
  });
});

// console.log(new Proxy(new Set(), {}))
Deno.test(function consoleLogShouldNotThrowErrorWhenInputIsProxiedSet() {
  mockConsole((console, out) => {
    const proxiedSet = new Proxy(new Set([1, 2]), {});
    console.log(proxiedSet);
    assertEquals(stripAnsiCode(out.toString()), "Set(2) { 1, 2 }\n");
  });
});

// console.log(new Proxy(new Map(), {}))
Deno.test(function consoleLogShouldNotThrowErrorWhenInputIsProxiedMap() {
  mockConsole((console, out) => {
    const proxiedMap = new Proxy(new Map([[1, 1], [2, 2]]), {});
    console.log(proxiedMap);
    assertEquals(stripAnsiCode(out.toString()), "Map(2) { 1 => 1, 2 => 2 }\n");
  });
});

// console.log(new Proxy(new Uint8Array(), {}))
Deno.test(function consoleLogShouldNotThrowErrorWhenInputIsProxiedTypedArray() {
  mockConsole((console, out) => {
    const proxiedUint8Array = new Proxy(new Uint8Array([1, 2]), {});
    console.log(proxiedUint8Array);
    assertEquals(stripAnsiCode(out.toString()), "Uint8Array(2) [ 1, 2 ]\n");
  });
});

// console.log(new Proxy(new RegExp(), {}))
Deno.test(function consoleLogShouldNotThrowErrorWhenInputIsProxiedRegExp() {
  mockConsole((console, out) => {
    const proxiedRegExp = new Proxy(/aaaa/, {});
    console.log(proxiedRegExp);
    assertEquals(stripAnsiCode(out.toString()), "/aaaa/\n");
  });
});

// console.log(new Proxy(new Date(), {}))
Deno.test(function consoleLogShouldNotThrowErrorWhenInputIsProxiedDate() {
  mockConsole((console, out) => {
    const proxiedDate = new Proxy(new Date("2022-09-24T15:59:39.529Z"), {});
    console.log(proxiedDate);
    assertEquals(stripAnsiCode(out.toString()), "2022-09-24T15:59:39.529Z\n");
  });
});

// console.log(new Proxy(new Error(), {}))
Deno.test(function consoleLogShouldNotThrowErrorWhenInputIsProxiedError() {
  mockConsole((console, out) => {
    const proxiedError = new Proxy(new Error("message"), {});
    console.log(proxiedError);
    assertStringIncludes(stripAnsiCode(out.toString()), "Error: message\n");
  });
});

// console.dir test
Deno.test(function consoleDir() {
  mockConsole((console, out) => {
    console.dir("DIR");
    assertEquals(out.toString(), "DIR\n");
  });
  mockConsole((console, out) => {
    console.dir("DIR", { indentLevel: 2 });
    assertEquals(out.toString(), "    DIR\n");
  });
});

// console.dir test
Deno.test(function consoleDirXml() {
  mockConsole((console, out) => {
    console.dirxml("DIRXML");
    assertEquals(out.toString(), "DIRXML\n");
  });
  mockConsole((console, out) => {
    console.dirxml("DIRXML", { indentLevel: 2 });
    assertEquals(out.toString(), "    DIRXML\n");
  });
});

// console.trace test
Deno.test(function consoleTrace() {
  mockConsole((console, _out, err) => {
    console.trace("%s", "custom message");
    assert(err);
    assert(err.toString().includes("Trace: custom message"));
  });
});

Deno.test(function inspectString() {
  assertEquals(
    stripAnsiCode(Deno.inspect("\0")),
    `"\\x00"`,
  );
  assertEquals(
    stripAnsiCode(Deno.inspect("\x1b[2J")),
    `"\\x1b[2J"`,
  );
});

Deno.test(function inspectGetters() {
  assertEquals(
    stripAnsiCode(Deno.inspect({
      get foo() {
        return 0;
      },
    })),
    "{ foo: [Getter] }",
  );

  assertEquals(
    stripAnsiCode(Deno.inspect({
      get foo() {
        return 0;
      },
    }, { getters: true })),
    "{ foo: [Getter: 0] }",
  );

  assertEquals(
    Deno.inspect({
      get foo() {
        throw new Error("bar");
      },
    }, { getters: true }),
    "{ foo: [Getter: <Inspection threw (bar)>] }",
  );
});

Deno.test(function inspectPrototype() {
  class A {}
  assertEquals(Deno.inspect(A.prototype), "{}");
});

Deno.test(function inspectSorted() {
  assertEquals(
    stripAnsiCode(Deno.inspect({ b: 2, a: 1 }, { sorted: true })),
    "{ a: 1, b: 2 }",
  );
  assertEquals(
    stripAnsiCode(Deno.inspect(new Set(["b", "a"]), { sorted: true })),
    `Set(2) { "a", "b" }`,
  );
  assertEquals(
    stripAnsiCode(Deno.inspect(
      new Map([
        ["b", 2],
        ["a", 1],
      ]),
      { sorted: true },
    )),
    `Map(2) { "a" => 1, "b" => 2 }`,
  );
});

Deno.test(function inspectTrailingComma() {
  assertEquals(
    stripAnsiCode(Deno.inspect(
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
    stripAnsiCode(Deno.inspect(
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
    stripAnsiCode(Deno.inspect(
      new Set([
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      ]),
      { trailingComma: true },
    )),
    `Set(2) {
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
}`,
  );
  assertEquals(
    stripAnsiCode(Deno.inspect(
      new Map([
        ["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 1],
        ["bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", 2],
      ]),
      { trailingComma: true },
    )),
    `Map(2) {
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" => 1,
  "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" => 2,
}`,
  );
});

Deno.test(function inspectCompact() {
  assertEquals(
    stripAnsiCode(Deno.inspect({ a: 1, b: 2 }, { compact: false })),
    `{
  a: 1,
  b: 2
}`,
  );
});

Deno.test(function inspectIterableLimit() {
  assertEquals(
    stripAnsiCode(Deno.inspect(["a", "b", "c"], { iterableLimit: 2 })),
    `[ "a", "b", ... 1 more item ]`,
  );
  assertEquals(
    stripAnsiCode(Deno.inspect(new Set(["a", "b", "c"]), { iterableLimit: 2 })),
    `Set(3) { "a", "b", ... 1 more item }`,
  );
  assertEquals(
    stripAnsiCode(Deno.inspect(
      new Map([
        ["a", 1],
        ["b", 2],
        ["c", 3],
      ]),
      { iterableLimit: 2 },
    )),
    `Map(3) { "a" => 1, "b" => 2, ... 1 more item }`,
  );
});

Deno.test(function inspectProxy() {
  assertEquals(
    stripAnsiCode(Deno.inspect(
      new Proxy([1, 2, 3], {}),
    )),
    "[ 1, 2, 3 ]",
  );
  assertEquals(
    stripAnsiCode(Deno.inspect(
      new Proxy({ key: "value" }, {}),
    )),
    `{ key: "value" }`,
  );
  assertEquals(
    stripAnsiCode(Deno.inspect(
      new Proxy({}, {
        get(_target, key) {
          if (key === Symbol.toStringTag) {
            return "MyProxy";
          } else {
            return 5;
          }
        },
        getOwnPropertyDescriptor() {
          return {
            enumerable: true,
            configurable: true,
            value: 5,
          };
        },
        ownKeys() {
          return ["prop1", "prop2"];
        },
      }),
    )),
    `Object [MyProxy] { prop1: 5, prop2: 5 }`,
  );
  assertEquals(
    stripAnsiCode(Deno.inspect(
      new Proxy([1, 2, 3], { get() {} }),
      { showProxy: true },
    )),
    "Proxy [ [ 1, 2, 3 ], { get: [Function: get] } ]",
  );
  assertEquals(
    stripAnsiCode(Deno.inspect(
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
    stripAnsiCode(Deno.inspect(
      new Proxy([1, 2, 3, 4, 5, 6, 7], { get() {} }),
      { showProxy: true },
    )),
    `Proxy [
  [
    1, 2, 3, 4,
    5, 6, 7
  ],
  { get: [Function: get] }
]`,
  );
  assertEquals(
    stripAnsiCode(Deno.inspect(
      new Proxy(function fn() {}, { get() {} }),
      { showProxy: true },
    )),
    "Proxy [ [Function: fn], { get: [Function: get] } ]",
  );
});

Deno.test(function inspectError() {
  const error1 = new Error("This is an error");
  const error2 = new Error("This is an error", {
    cause: new Error("This is a cause error"),
  });

  assertStringIncludes(
    stripAnsiCode(Deno.inspect(error1)),
    "Error: This is an error",
  );
  assertStringIncludes(
    stripAnsiCode(Deno.inspect(error2)),
    "Error: This is an error",
  );
  assertStringIncludes(
    stripAnsiCode(Deno.inspect(error2)),
    "Caused by Error: This is a cause error",
  );
});

Deno.test(function inspectErrorCircular() {
  const error1 = new Error("This is an error");
  const error2 = new Error("This is an error", {
    cause: new Error("This is a cause error"),
  });
  error1.cause = error1;
  assert(error2.cause instanceof Error);
  error2.cause.cause = error2;

  assertStringIncludes(
    stripAnsiCode(Deno.inspect(error1)),
    "Error: This is an error",
  );
  assertStringIncludes(
    stripAnsiCode(Deno.inspect(error2)),
    "<ref *1> Error: This is an error",
  );
  assertStringIncludes(
    stripAnsiCode(Deno.inspect(error2)),
    "Caused by Error: This is a cause error",
  );
  assertStringIncludes(
    stripAnsiCode(Deno.inspect(error2)),
    "Caused by [Circular *1]",
  );
});

Deno.test(function inspectErrorWithCauseFormat() {
  const error = new Error("This is an error", {
    cause: {
      code: 100500,
    },
  });
  assertStringIncludes(
    stripAnsiCode(Deno.inspect(error)),
    "Error: This is an error",
  );
  assertStringIncludes(
    stripAnsiCode(Deno.inspect(error)),
    "Caused by { code: 100500 }",
  );
});

Deno.test(function inspectColors() {
  assertEquals(Deno.inspect(1), "1");
  assertStringIncludes(Deno.inspect(1, { colors: true }), "\x1b[");
});

Deno.test(function inspectEmptyArray() {
  const arr: string[] = [];

  assertEquals(
    Deno.inspect(arr, {
      compact: false,
      trailingComma: true,
    }),
    "[]",
  );
});

Deno.test(function inspectDeepEmptyArray() {
  const obj = {
    arr: [],
  };

  assertEquals(
    Deno.inspect(obj, {
      compact: false,
      trailingComma: true,
    }),
    `{
  arr: [],
}`,
  );
});

Deno.test(function inspectEmptyMap() {
  const map = new Map();

  assertEquals(
    Deno.inspect(map, {
      compact: false,
      trailingComma: true,
    }),
    "Map(0) {}",
  );
});

Deno.test(function inspectEmptySet() {
  const set = new Set();

  assertEquals(
    Deno.inspect(set, {
      compact: false,
      trailingComma: true,
    }),
    "Set(0) {}",
  );
});

Deno.test(function inspectEmptyUint8Array() {
  const typedArray = new Uint8Array(0);

  assertEquals(
    Deno.inspect(typedArray, {
      compact: false,
      trailingComma: true,
    }),
    "Uint8Array(0) []",
  );
});

Deno.test(function inspectLargeArrayBuffer() {
  const arrayBuffer = new ArrayBuffer(2 ** 32 + 1);
  assertEquals(
    Deno.inspect(arrayBuffer),
    `ArrayBuffer {
  [Uint8Contents]: <00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 ... 4294967197 more bytes>,
  byteLength: 4294967297
}`,
  );
  structuredClone(arrayBuffer, { transfer: [arrayBuffer] });
  assertEquals(
    Deno.inspect(arrayBuffer),
    "ArrayBuffer { (detached), byteLength: 0 }",
  );

  const sharedArrayBuffer = new SharedArrayBuffer(2 ** 32 + 1);
  assertEquals(
    Deno.inspect(sharedArrayBuffer),
    `SharedArrayBuffer {
  [Uint8Contents]: <00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 ... 4294967197 more bytes>,
  byteLength: 4294967297
}`,
  );
});

Deno.test(function inspectStringAbbreviation() {
  const LONG_STRING =
    "This is a really long string which will be abbreviated with ellipsis.";
  const obj = {
    str: LONG_STRING,
  };
  const arr = [LONG_STRING];

  assertEquals(
    Deno.inspect(obj, { strAbbreviateSize: 10 }),
    '{ str: "This is a "... 59 more characters }',
  );

  assertEquals(
    Deno.inspect(arr, { strAbbreviateSize: 10 }),
    '[ "This is a "... 59 more characters ]',
  );
});

Deno.test(async function inspectAggregateError() {
  try {
    await Promise.any([]);
  } catch (err) {
    assertEquals(
      Deno.inspect(err).trimEnd(),
      "AggregateError: All promises were rejected",
    );
  }
});

Deno.test(function inspectWithPrototypePollution() {
  const originalExec = RegExp.prototype.exec;
  try {
    RegExp.prototype.exec = () => {
      throw Error();
    };
    Deno.inspect("foo");
  } finally {
    RegExp.prototype.exec = originalExec;
  }
});

Deno.test(function inspectPromiseLike() {
  assertEquals(
    Deno.inspect(Object.create(Promise.prototype)),
    "Promise {}",
  );
});

Deno.test(function inspectorMethods() {
  console.timeStamp("test");
  console.profile("test");
  console.profileEnd("test");
});

Deno.test(function inspectQuotesOverride() {
  assertEquals(
    // @ts-ignore - 'quotes' is an internal option
    Deno.inspect("foo", { quotes: ["'", '"', "`"] }),
    "'foo'",
  );
  assertEquals(
    // @ts-ignore - 'quotes' is an internal option
    Deno.inspect("'foo'", { quotes: ["'", '"', "`"] }),
    `"'foo'"`,
  );
});

Deno.test(function inspectAnonymousFunctions() {
  assertEquals(Deno.inspect(() => {}), "[Function (anonymous)]");
  assertEquals(Deno.inspect(function () {}), "[Function (anonymous)]");
  assertEquals(Deno.inspect(async () => {}), "[AsyncFunction (anonymous)]");
  assertEquals(
    Deno.inspect(async function () {}),
    "[AsyncFunction (anonymous)]",
  );
  assertEquals(
    Deno.inspect(function* () {}),
    "[GeneratorFunction (anonymous)]",
  );
  assertEquals(
    Deno.inspect(async function* () {}),
    "[AsyncGeneratorFunction (anonymous)]",
  );
});

Deno.test(function inspectBreakLengthOption() {
  assertEquals(
    Deno.inspect("123456789\n".repeat(3), { breakLength: 34 }),
    `"123456789\\n123456789\\n123456789\\n"`,
  );
  assertEquals(
    Deno.inspect("123456789\n".repeat(3), { breakLength: 33 }),
    `"123456789\\n" +
  "123456789\\n" +
  "123456789\\n"`,
  );
});

Deno.test(function inspectEscapeSequencesFalse() {
  assertEquals(
    Deno.inspect("foo\nbar", { escapeSequences: true }),
    '"foo\\nbar"',
  ); // default behavior
  assertEquals(
    Deno.inspect("foo\nbar", { escapeSequences: false }),
    '"foo\nbar"',
  );
});
