// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { isContext, runInNewContext } from "node:vm";
import {
  assertEquals,
  assertThrows,
} from "../../../test_util/std/assert/mod.ts";

Deno.test({
  name: "vm runInNewContext",
  fn() {
    const two = runInNewContext("1 + 1");
    assertEquals(two, 2);
  },
});

Deno.test({
  name: "vm runInNewContext sandbox",
  fn() {
    assertThrows(() => runInNewContext("Deno"));
    // deno-lint-ignore no-var
    var a = 1;
    assertThrows(() => runInNewContext("a + 1"));

    runInNewContext("a = 2");
    assertEquals(a, 1);
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
