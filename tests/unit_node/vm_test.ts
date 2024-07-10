// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "@std/assert/mod.ts";
import {
  createContext,
  isContext,
  runInContext,
  runInNewContext,
  runInThisContext,
  Script,
} from "node:vm";

Deno.test({
  name: "vm runInNewContext",
  fn() {
    const two = runInNewContext("1 + 1");
    assertEquals(two, 2);
  },
});

Deno.test({
  name: "vm new Script()",
  fn() {
    const script = new Script(`
function add(a, b) {
  return a + b;
}
const x = add(1, 2);
x
`);

    const value = script.runInThisContext();
    assertEquals(value, 3);
  },
});

// https://github.com/denoland/deno/issues/23186
Deno.test({
  name: "vm runInNewContext sandbox",
  fn() {
    const sandbox = { fromAnotherRealm: false };
    runInNewContext("fromAnotherRealm = {}", sandbox);

    assertEquals(typeof sandbox.fromAnotherRealm, "object");
  },
});

// https://github.com/denoland/deno/issues/22395
Deno.test({
  name: "vm runInewContext with context object",
  fn() {
    const context = { a: 1, b: 2 };
    const result = runInNewContext("a + b", context);
    assertEquals(result, 3);
  },
});

// https://github.com/denoland/deno/issues/18299
Deno.test({
  name: "vm createContext and runInContext",
  fn() {
    // @ts-expect-error implicit any
    globalThis.globalVar = 3;

    const context = { globalVar: 1 };
    createContext(context);
    runInContext("globalVar *= 2", context);
    assertEquals(context.globalVar, 2);
    // @ts-expect-error implicit any
    assertEquals(globalThis.globalVar, 3);
  },
});

Deno.test({
  name: "vm runInThisContext Error rethrow",
  fn() {
    assertThrows(
      () => {
        runInThisContext("throw new Error('error')");
      },
      Error,
      "error",
    );
    assertThrows(
      () => {
        runInThisContext("throw new TypeError('type error')");
      },
      TypeError,
      "type error",
    );
  },
});

// https://github.com/webpack/webpack/blob/87660921808566ef3b8796f8df61bd79fc026108/lib/javascript/JavascriptParser.js#L4329
Deno.test({
  name: "vm runInNewContext webpack magic comments",
  fn() {
    const webpackCommentRegExp = new RegExp(
      /(^|\W)webpack[A-Z]{1,}[A-Za-z]{1,}:/,
    );
    const comments = [
      'webpackChunkName: "test"',
      'webpackMode: "lazy"',
      "webpackPrefetch: true",
      "webpackPreload: true",
      "webpackProvidedExports: true",
      'webpackChunkLoading: "require"',
      'webpackExports: ["default", "named"]',
    ];

    for (const comment of comments) {
      const result = webpackCommentRegExp.test(comment);
      assertEquals(result, true);

      const [[key, _value]]: [string, string][] = Object.entries(
        runInNewContext(`(function(){return {${comment}};})()`),
      );
      const expectedKey = comment.split(":")[0].trim();
      assertEquals(key, expectedKey);
    }
  },
});

// https://github.com/denoland/deno/issues/18315
Deno.test({
  name: "vm isContext",
  fn() {
    // Currently we do not expose VM contexts so this is always false.
    const obj = {};
    assertEquals(isContext(obj), false);
    assertEquals(isContext(globalThis), false);
    const sandbox = runInNewContext("{}");
    assertEquals(isContext(sandbox), false);
  },
});

// https://github.com/denoland/deno/issues/23297
Deno.test({
  name: "vm context promise rejection",
  fn() {
    const code = `
function reject() {
  return Promise.reject(new Error('rejected'));
}
reject().catch(() => {})
    `;

    const script = new Script(code);
    script.runInNewContext();
  },
});

// https://github.com/denoland/deno/issues/22441
Deno.test({
  name: "vm runInNewContext module loader",
  fn() {
    const code = "import('node:process')";
    const script = new Script(code);
    script.runInNewContext();
  },
});

// https://github.com/denoland/deno/issues/23913
Deno.test({
  name: "vm memory leak crash",
  fn() {
    const script = new Script("returnValue = 2+2");

    for (let i = 0; i < 1000; i++) {
      script.runInNewContext({}, { timeout: 10000 });
    }
  },
});

// https://github.com/denoland/deno/issues/23852
Deno.test({
  name: "vm runInThisContext global.foo",
  fn() {
    const result = runInThisContext(`global.foo = 1`);
    assertEquals(result, 1);
  },
});
