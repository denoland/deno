// Copyright 2018-2026 the Deno authors. MIT license.

// Tests for tools/lint_plugins/prefer_primordials.ts
// Ported from deno_lint's prefer_primordials rule.
//
// Original cases derived from
// https://github.com/nodejs/node/blob/7919ced0c97e9a5b17e6042e0b57bc911d23583d/test/parallel/test-eslint-prefer-primordials.js
//
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

import { assertEquals } from "./test_util.ts";
import plugin, {
  HINT,
  MSG,
} from "../../tools/lint_plugins/prefer_primordials.ts";

const RULE_ID = "deno-internal/prefer-primordials";

function lint(source: string, fileName = "source.ts"): Deno.lint.Diagnostic[] {
  return Deno.lint.runPlugin(plugin, fileName, source);
}

function assertOk(source: string) {
  const diags = lint(source);
  assertEquals(
    diags,
    [],
    `Expected no diagnostics, got:\n${
      diags.map((d) => `  ${d.message} @${d.range}`).join("\n")
    }`,
  );
}

function assertErr(
  source: string,
  expected: Array<{ message: string; hint?: string }>,
) {
  const diags = lint(source).slice().sort((a, b) =>
    a.range[0] - b.range[0] || a.range[1] - b.range[1]
  );
  assertEquals(
    diags.length,
    expected.length,
    `Expected ${expected.length} diagnostics, got ${diags.length}:\n${
      diags.map((d) => `  [${d.id}] ${d.message} hint=${d.hint} @${d.range}`)
        .join("\n")
    }\nSource:\n${source}`,
  );
  for (let i = 0; i < expected.length; i++) {
    assertEquals(diags[i].id, RULE_ID);
    assertEquals(diags[i].message, expected[i].message);
    if (expected[i].hint !== undefined) {
      assertEquals(diags[i].hint, expected[i].hint);
    }
  }
}

Deno.test(function preferPrimordialsValid() {
  assertOk(`
const { Array } = primordials;
new Array();
  `);
  assertOk(`
const { JSONStringify } = primordials;
JSONStringify({});
  `);
  assertOk(`
const { SymbolFor } = primordials;
SymbolFor("foo");
  `);
  assertOk(`
const { SymbolIterator } = primordials;
class A {
  *[SymbolIterator] () {
    yield "a";
  }
}
  `);
  assertOk(`
const { SymbolIterator } = primordials;
const a = {
  *[SymbolIterator] () {
    yield "a";
  }
}
  `);
  assertOk(`
const { ObjectDefineProperty, SymbolToStringTag } = primordials;
ObjectDefineProperty(o, SymbolToStringTag, { __proto__: null, value: "o" });
  `);
  assertOk(`
const { ReflectDefineProperty, SymbolToStringTag } = primordials;
ReflectDefineProperty(o, SymbolToStringTag, { __proto__: null, value: "o" });
  `);
  assertOk(`
const { ObjectDefineProperties } = primordials;
ObjectDefineProperties(o, {
  foo: { __proto__: null, value: "o" },
  bar: { "__proto__": null, value: "o" },
});
  `);
  assertOk(`
function foo(o = { __proto__: null }) {}
function bar({ o = { __proto__: null } }) {}
  `);
  assertOk(`
const { NumberParseInt } = primordials;
NumberParseInt("42");
  `);
  assertOk(`
const { ReflectOwnKeys } = primordials;
ReflectOwnKeys({});
  `);
  assertOk(`
const { SafeRegExp } = primordials;
new SafeRegExp("aaaa");
  `);
  assertOk(`
const { SafeMap } = primordials;
new SafeMap();
  `);
  assertOk(`
const { SafePromiseAll, PromiseResolve } = primordials;
SafePromiseAll([
  PromiseResolve(1),
  PromiseResolve(2),
]);
  `);
  assertOk(`
const { ArrayPrototypeMap } = primordials;
ArrayPrototypeMap([1, 2, 3], (val) => val * 2);
  `);
  assertOk(`
const parseInt = () => {};
parseInt();
  `);
  assertOk(`const foo = { Error: 1 };`);
  assertOk(`foo.description = 1`);
  assertOk(`foo.description()`);
  assertOk(`
const { SafeRegExp } = primordials;
const pattern = new SafeRegExp(/aaaa/u);
pattern.source;
  `);
  assertOk(`
const { SafeSet } = primordials;
const set = new SafeSet();
set.add(1);
set.add(2);
set.size;
  `);
  assertOk(`
const foo = { size: 100 };
foo.size;
  `);
  assertOk(`
const { SafeArrayIterator } = primordials;
[1, 2, ...new SafeArrayIterator(arr)];
foo(1, 2, ...new SafeArrayIterator(arr));
new Foo(1, 2, ...new SafeArrayIterator(arr));
  `);
  assertOk(`
const { SafeArrayIterator } = primordials;
[1, 2, ...new SafeArrayIterator([1, 2, 3])];
foo(1, 2, ...new SafeArrayIterator([1, 2, 3]));
new Foo(1, 2, ...new SafeArrayIterator([1, 2, 3]));
  `);
  assertOk(`
({ ...{} });
  `);
  assertOk(`
const { SafeArrayIterator } = primordials;
for (const val of new SafeArrayIterator(arr)) {}
for (const val of new SafeArrayIterator([1, 2, 3])) {}
  `);
  assertOk(`
const { SafeArrayIterator } = primordials;
function* foo() { yield* new SafeArrayIterator([1, 2, 3]); }
  `);
  assertOk(`
const { 0: a, 1: b } = [1, 2];
  `);
  assertOk(`
let a, b;
({ 0: a, 1: b } = [1, 2]);
  `);
  assertOk(`
const { SafeArrayIterator } = primordials;
const [a, b, ...c] = new SafeArrayIterator([1, 2, 3]);
  `);
  assertOk(`
const { SafeArrayIterator } = primordials;
let a, b, c;
[a, b, ...c] = new SafeArrayIterator([1, 2, 3]);
  `);
  assertOk(`
const { indirectEval } = primordials;
indirectEval("console.log('This test should pass.');");
  `);
  assertOk(`
function foo(a: Array<any>) {}
  `);
  assertOk(`
function foo(): Array<any> {}
  `);
  assertOk(`
type p = Promise<void>;
  `);
  // Class field names are bindings, not free globals.
  assertOk(`class A { static RangeError = 1; }`);
  // Type-only property names have no runtime pollution surface.
  assertOk(`interface Opts { eval?: boolean; }`);
  // Primordials destructuring must shadow globals inside IIFEs / nested scopes.
  assertOk(`
(function () {
  const { TypeError } = primordials;
  throw new TypeError("x");
})();
  `);
  // Private brand checks are the safe `in` form (deno_lint#1308).
  assertOk(`
class A {
  #brand;

  static is(obj) {
    return #brand in obj;
  }
}
  `);
});

Deno.test(function preferPrimordialsInvalid() {
  assertErr(`const foo = Symbol("foo");`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  assertErr(`const foo = Symbol.for("foo");`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  assertErr(`const arr = new Array();`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  assertErr(
    `
const { RegExp } = primordials;
new RegExp("aaaa");
  `,
    [
      { message: MSG.UnsafeIntrinsic, hint: HINT.UnsafeIntrinsic },
    ],
  );
  assertErr(
    `
const { Map } = primordials;
new Map();
  `,
    [
      { message: MSG.UnsafeIntrinsic, hint: HINT.UnsafeIntrinsic },
    ],
  );
  assertErr(
    `
const { PromiseAll, PromiseResolve } = primordials;
PromiseAll([
  PromiseResolve(1),
  PromiseResolve(2),
]);
  `,
    [
      { message: MSG.UnsafeIntrinsic, hint: HINT.UnsafeIntrinsic },
    ],
  );
  assertErr(`JSON.parse("{}")`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  assertErr(
    `
const { JSON } = primordials;
JSON.parse("{}");
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(`Symbol.for("foo")`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  assertErr(
    `
const { Symbol } = primordials;
Symbol.for("foo");
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(
    `
const { Symbol } = primordials;
class A {
  *[Symbol.iterator] () {
    yield "a";
  }
}
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(
    `
const { Symbol } = primordials;
const a = {
  *[Symbol.iterator] () {
    yield "a";
  }
}
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  // A member expression in a computed property is not part of the outer
  // member chain, so its global root must still be checked.
  assertErr(`const it = obj[Symbol.iterator];`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  assertErr(
    `
const { ObjectDefineProperty, Symbol } = primordials;
ObjectDefineProperty(o, Symbol.toStringTag, { value: "o" });
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
      { message: MSG.DefineProperty, hint: HINT.NullPrototypeObjectLiteral },
    ],
  );
  assertErr(
    `
const { ObjectDefineProperty, SymbolToStringTag } = primordials;
ObjectDefineProperty(o, SymbolToStringTag, { value: "o" });
  `,
    [
      { message: MSG.DefineProperty, hint: HINT.NullPrototypeObjectLiteral },
    ],
  );
  assertErr(
    `
const { ObjectDefineProperties } = primordials;
ObjectDefineProperties(o, {
  foo: { value: "o" },
  bar: { __proto__: {}, value: "o" },
  baz: { ["__proto__"]: null, value: "o" },
});
  `,
    [
      { message: MSG.DefineProperty, hint: HINT.NullPrototypeObjectLiteral },
      { message: MSG.DefineProperty, hint: HINT.NullPrototypeObjectLiteral },
      { message: MSG.DefineProperty, hint: HINT.NullPrototypeObjectLiteral },
    ],
  );
  assertErr(
    `
function foo(o = {}) {}
function bar({ o = {} }) {}
  `,
    [
      {
        message: MSG.ObjectAssignInDefaultParameter,
        hint: HINT.NullPrototypeObjectLiteral,
      },
      {
        message: MSG.ObjectAssignInDefaultParameter,
        hint: HINT.NullPrototypeObjectLiteral,
      },
    ],
  );
  assertErr(
    `
const { Number } = primordials;
Number.parseInt("10");
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(`parseInt("10")`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  assertErr(`const { ownKeys } = Reflect;`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  assertErr(`new Map();`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    { message: MSG.UnsafeIntrinsic, hint: HINT.UnsafeIntrinsic },
  ]);
  assertErr(
    `
const { Function } = primordials;
const noop = Function.prototype;
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(`[1, 2, 3].map(val => val * 2);`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  assertErr(
    `
const obj = { foo: 1 };
obj.hasOwnProperty("foo");
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(
    `
const fn = () => 1;
fn.call(null);
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(
    `
const num = 123.456;
num.toFixed(2);
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(
    `
const { Date } = primordials;
new Date().toISOString();
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(
    `
const arr = [1, 2, 3, 4];
arr.filter((val) => val % 2 === 0);
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(
    `
const str = "foo bar baz";
str.split(" ");
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(
    `
const thenable = { then() {} };
thenable.then(() => {});
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(
    `
const { Uint8Array } = primordials;
new Uint8Array(10).buffer;
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(
    `
const { ArrayBuffer } = primordials;
new ArrayBuffer(10).byteLength;
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(
    `
const { ArrayBuffer, DataView } = primordials;
new DataView(new ArrayBuffer(10)).byteOffset;
  `,
    [
      { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
    ],
  );
  assertErr(`foo = bar.description;`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  assertErr(`"a" in A`, [
    { message: MSG.In, hint: HINT.In },
  ]);
  assertErr(`a in A`, [
    { message: MSG.In, hint: HINT.In },
  ]);
  assertErr(`a instanceof A`, [
    { message: MSG.InstanceOf, hint: HINT.InstanceOf },
  ]);
  assertErr(`[1, 2, ...arr];`, [
    { message: MSG.Iterator, hint: HINT.SafeIterator },
  ]);
  assertErr(`foo(1, 2, ...arr);`, [
    { message: MSG.Iterator, hint: HINT.SafeIterator },
  ]);
  assertErr(`new Foo(1, 2, ...arr);`, [
    { message: MSG.Iterator, hint: HINT.SafeIterator },
  ]);
  assertErr(`[1, 2, ...[3]];`, [
    { message: MSG.Iterator, hint: HINT.SafeIterator },
  ]);
  assertErr(`foo(1, 2, ...[3]);`, [
    { message: MSG.Iterator, hint: HINT.SafeIterator },
  ]);
  assertErr(`new Foo(1, 2, ...[3]);`, [
    { message: MSG.Iterator, hint: HINT.SafeIterator },
  ]);
  assertErr(`for (const val of arr) {}`, [
    { message: MSG.Iterator, hint: HINT.SafeIterator },
  ]);
  assertErr(`for (const val of [1, 2, 3]) {}`, [
    { message: MSG.Iterator, hint: HINT.SafeIterator },
  ]);
  assertErr(`function* foo() { yield* [1, 2, 3]; }`, [
    { message: MSG.Iterator, hint: HINT.SafeIterator },
  ]);
  assertErr(`const [a, b] = [1, 2];`, [
    { message: MSG.Iterator, hint: HINT.ObjectPattern },
  ]);
  assertErr(
    `
let a, b;
[a, b] = [1, 2];
  `,
    [
      { message: MSG.Iterator, hint: HINT.ObjectPattern },
    ],
  );
  assertErr(`const [a, b, ...c] = [1, 2, 3];`, [
    { message: MSG.Iterator, hint: HINT.SafeIterator },
  ]);
  assertErr(
    `
let a, b, c;
[a, b, ...c] = [1, 2, 3];
  `,
    [
      { message: MSG.Iterator, hint: HINT.SafeIterator },
    ],
  );
  assertErr(`/aaa/u`, [
    { message: MSG.RegExp, hint: HINT.SafeRegExp },
  ]);
  assertErr(`eval("console.log('This test should fail!');");`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  // Optional method calls still perform prototype lookup — flag them.
  assertErr(`foo.unshift?.(x);`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
  // Getter used as a call argument must still be flagged (not only as callee).
  assertErr(`fn(obj.buffer);`, [
    { message: MSG.GlobalIntrinsic, hint: HINT.GlobalIntrinsic },
  ]);
});
