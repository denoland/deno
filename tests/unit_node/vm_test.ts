// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { createContext, isContext, runInContext, runInNewContext, Script } from "node:vm";
import { assertEquals, assertThrows } from "@std/assert/mod.ts";

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

Deno.test({
  name: "vm runInNewContext sandbox",
  fn() {
    // deno-lint-ignore no-var
    var a = 1;
    assertThrows(() => runInNewContext("a + 1"));

    runInNewContext("a = 2");
    assertEquals(a, 1);
  },
});

Deno.test({
  name: "vm createContext",
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
