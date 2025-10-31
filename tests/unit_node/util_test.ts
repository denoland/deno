// Copyright 2018-2025 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertStrictEquals,
  assertThrows,
} from "@std/assert";
import { stripAnsiCode } from "@std/fmt/colors";
import * as util from "node:util";
import utilDefault from "node:util";
import { Buffer } from "node:buffer";

Deno.test({
  name: "[util] format",
  fn() {
    assertEquals(util.format("%o", [10, 11]), "[ 10, 11, [length]: 2 ]");
  },
});

Deno.test({
  name: "[util] inspect.custom",
  fn() {
    assertEquals(util.inspect.custom, Symbol.for("nodejs.util.inspect.custom"));
  },
});

Deno.test({
  name: "[util] inspect",
  fn() {
    assertEquals(stripAnsiCode(util.inspect({ foo: 123 })), "{ foo: 123 }");
    assertEquals(stripAnsiCode(util.inspect("foo")), "'foo'");
    assertEquals(
      stripAnsiCode(util.inspect("Deno's logo is so cute.")),
      `"Deno's logo is so cute."`,
    );
    assertEquals(
      stripAnsiCode(util.inspect([1, 2, 3, 4, 5, 6, 7])),
      `[
  1, 2, 3, 4,
  5, 6, 7
]`,
    );
  },
});

Deno.test({
  name: "[util] types.isTypedArray",
  fn() {
    assert(util.types.isTypedArray(new Buffer(4)));
    assert(util.types.isTypedArray(new Uint8Array(4)));
    assert(!util.types.isTypedArray(new DataView(new ArrayBuffer(4))));
  },
});

Deno.test({
  name: "[util] types.isNativeError",
  fn() {
    assert(util.types.isNativeError(new Error()));
    assert(util.types.isNativeError(new TypeError()));
    assert(util.types.isNativeError(new DOMException()));
  },
});

Deno.test({
  name: "[util] TextDecoder",
  fn() {
    assert(util.TextDecoder === TextDecoder);
    const td: util.TextDecoder = new util.TextDecoder();
    assert(td instanceof TextDecoder);
  },
});

Deno.test({
  name: "[util] TextEncoder",
  fn() {
    assert(util.TextEncoder === TextEncoder);
    const te: util.TextEncoder = new util.TextEncoder();
    assert(te instanceof TextEncoder);
  },
});

Deno.test({
  name: "[util] toUSVString",
  fn() {
    assertEquals(util.toUSVString("foo"), "foo");
    assertEquals(util.toUSVString("bar\ud801"), "bar\ufffd");
  },
});

Deno.test({
  name: "[util] getSystemErrorName()",
  fn() {
    type FnTestInvalidArg = (code?: unknown) => void;

    assertThrows(
      () => (util.getSystemErrorName as FnTestInvalidArg)(),
      TypeError,
    );
    assertThrows(
      () => (util.getSystemErrorName as FnTestInvalidArg)(1),
      RangeError,
    );

    assertStrictEquals(util.getSystemErrorName(-424242), undefined);

    switch (Deno.build.os) {
      case "windows":
        assertStrictEquals(util.getSystemErrorName(-4091), "EADDRINUSE");
        break;

      case "darwin":
        assertStrictEquals(util.getSystemErrorName(-48), "EADDRINUSE");
        break;

      case "linux":
        assertStrictEquals(util.getSystemErrorName(-98), "EADDRINUSE");
        break;
    }
  },
});

Deno.test({
  name: "[util] getSystemErrorMessage()",
  fn() {
    type FnTestInvalidArg = (code?: unknown) => void;

    assertThrows(
      () => (util.getSystemErrorMessage as FnTestInvalidArg)(),
      TypeError,
    );
    assertThrows(
      () => (util.getSystemErrorMessage as FnTestInvalidArg)(1),
      RangeError,
    );

    assertStrictEquals(util.getSystemErrorMessage(-424242), undefined);

    switch (Deno.build.os) {
      case "windows":
        assertStrictEquals(
          util.getSystemErrorMessage(-4091),
          "address already in use",
        );
        break;
      case "darwin":
        assertStrictEquals(
          util.getSystemErrorMessage(-48),
          "address already in use",
        );
        break;
      case "linux":
        assertStrictEquals(
          util.getSystemErrorMessage(-98),
          "address already in use",
        );
        break;
    }
  },
});

Deno.test({
  name: "[util] deprecate() works",
  fn() {
    const fn = util.deprecate(() => {}, "foo");
    fn();
  },
});

Deno.test({
  name: "[util] callbackify() works",
  fn() {
    const fn = util.callbackify(() => Promise.resolve("foo"));
    fn((err, value) => {
      assert(err === null);
      assert(value === "foo");
    });
  },
});

Deno.test({
  name: "[util] callbackify(undefined) throws",
  fn() {
    assertThrows(
      // @ts-expect-error: testing runtime error
      () => util.callbackify(undefined),
      TypeError,
      'The "original" argument must be of type function',
    );
  },
});

Deno.test({
  name: "[util] parseArgs() with no args works",
  fn() {
    util.parseArgs({});
  },
});

Deno.test("[util] debuglog() and debug()", () => {
  assert(typeof util.debug === "function");
  assert(typeof util.debuglog === "function");
  assertEquals(util.debuglog, util.debug);
  assertEquals(utilDefault.debuglog, utilDefault.debug);
});

Deno.test("[util] aborted()", async () => {
  const abortController = new AbortController();
  let done = false;
  const promise = util.aborted(
    // deno-lint-ignore no-explicit-any
    abortController.signal as any,
    abortController.signal,
  );
  promise.then(() => {
    done = true;
  });
  await new Promise((r) => setTimeout(r, 100));
  assertEquals(done, false);
  abortController.abort();
  await promise;
  assertEquals(done, true);
});

Deno.test("[util] styleText()", () => {
  const redText = util.styleText("red", "error");
  assertEquals(redText, "\x1B[31merror\x1B[39m");
});

Deno.test("[util] styleText() with array of formats", () => {
  const colored = util.styleText(["red", "green"], "error");
  assertEquals(colored, "\x1b[32m\x1b[31merror\x1b[39m\x1b[39m");
});
