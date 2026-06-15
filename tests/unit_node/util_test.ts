// Copyright 2018-2026 the Deno authors. MIT license.

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
import process from "node:process";

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
  // https://github.com/denoland/deno/issues/26355
  name: "[util] inspect on Proxy doesn't invoke traps",
  fn() {
    assertEquals(
      stripAnsiCode(util.inspect(
        // deno-lint-ignore no-explicit-any
        new Proxy({ x: 1 }, { ownKeys: (() => undefined) as any }),
      )),
      "{ x: 1 }",
    );
    assertEquals(
      stripAnsiCode(util.inspect(
        new Proxy({}, {
          get() {
            throw new Error("should not be invoked");
          },
        }),
      )),
      "{}",
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

Deno.test("[util] aborted() drops pending promise when resource is GCed", async () => {
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--quiet",
      "--v8-flags=--expose-gc",
      "tests/unit_node/testdata/util_aborted_gc.ts",
    ],
    stdout: "piped",
    stderr: "piped",
  });
  const { code, stderr } = await command.output();
  assertEquals(code, 0, new TextDecoder().decode(stderr));
});

Deno.test("[util] styleText()", () => {
  const redText = util.styleText("red", "error", { validateStream: false });
  assertEquals(redText, "\x1B[31merror\x1B[39m");
});

Deno.test("[util] styleText() with array of formats", () => {
  const colored = util.styleText(["red", "green"], "error", {
    validateStream: false,
  });
  assertEquals(colored, "\x1b[31m\x1b[32merror\x1b[39m\x1b[39m");
});

Deno.test("[util] styleText() respects stream.isTTY", () => {
  const streamTTY = {
    write() {},
    isTTY: true,
  } as unknown as NodeJS.WritableStream;
  const streamNoTTY = {
    write() {},
    isTTY: false,
  } as unknown as NodeJS.WritableStream;

  const redText = util.styleText("red", "TTY", { stream: streamTTY });
  assertEquals(redText, "\x1b[31mTTY\x1b[39m");

  const plainText = util.styleText("blue", "No TTY", { stream: streamNoTTY });
  const greenText = util.styleText("green", "No TTY", {
    stream: streamNoTTY,
    validateStream: false,
  });
  assertEquals(plainText, "No TTY");
  assertEquals(greenText, "\x1b[32mNo TTY\x1b[39m");
});

Deno.test("[util] styleText() falls back to process.stdout when no stream given", () => {
  const orig = process.env.FORCE_COLOR;
  try {
    process.env.FORCE_COLOR = "0";
    assertEquals(util.styleText("red", "no stream"), "no stream");

    process.env.FORCE_COLOR = "1";
    assertEquals(
      util.styleText("red", "no stream"),
      "\x1b[31mno stream\x1b[39m",
    );
  } finally {
    if (orig === undefined) {
      delete process.env.FORCE_COLOR;
    } else {
      process.env.FORCE_COLOR = orig;
    }
  }
});

Deno.test("[util] stripVTControlCharacters() removes OSC 8 hyperlinks", () => {
  // OSC 8 hyperlink with ESC \ (ST) terminator
  const input =
    "\x1b]8;;http://example.com\x1b\\This is a link\x1b]8;;\x1b\\ hello";
  assertEquals(util.stripVTControlCharacters(input), "This is a link hello");

  // OSC 8 hyperlink with BEL terminator
  const inputBel =
    "\x1b]8;;http://example.com\x07This is a link\x1b]8;;\x07 hello";
  assertEquals(util.stripVTControlCharacters(inputBel), "This is a link hello");
});

Deno.test("[util] queryObjects() counts instances", () => {
  class UtilQueryObjectsFixture {}
  // util.queryObjects is not declared on the bundled @types/node yet, but the
  // runtime exposes it (mirroring v8.queryObjects).
  // deno-lint-ignore no-explicit-any
  const queryObjects = (util as any).queryObjects as (
    ctor: unknown,
    options?: { format?: "count" | "summary" },
  ) => number | string[];
  const before = queryObjects(UtilQueryObjectsFixture, { format: "count" });
  const refs = [];
  for (let i = 0; i < 25; i++) refs.push(new UtilQueryObjectsFixture());
  const after = queryObjects(UtilQueryObjectsFixture, { format: "count" });
  assertEquals(typeof before, "number");
  assertEquals((after as number) - (before as number) >= 25, true);
  assertEquals(refs.length, 25);
});

Deno.test("[util] parseEnv()", () => {
  const env =
    "KEY1=VALUE1\nKEY2='VALUE2'\nKEYÄ3=\"VALUE3\"\nKEY4=VALÜE4\nKEY5='VALUE6'INVALID_LINE\nKEY6=A";
  const parsed = util.parseEnv(env);
  assertEquals(parsed, {
    KEY1: "VALUE1",
    KEY2: "VALUE2",
    KEYÄ3: "VALUE3",
    KEY4: "VALÜE4",
    KEY5: "VALUE6",
    KEY6: "A",
  });
});

Deno.test("[util] getSystemErrorMap()", () => {
  const map = util.getSystemErrorMap();
  assert(map instanceof Map);
  // The map must agree with getSystemErrorName / getSystemErrorMessage on
  // every entry it returns.
  for (const [code, [name, message]] of map) {
    assertStrictEquals(util.getSystemErrorName(code), name);
    assertStrictEquals(util.getSystemErrorMessage(code), message);
  }
  // Smoke-check a couple of well-known platform-independent codes.
  const eaddrinuse = Deno.build.os === "windows" ? -4091 : -98;
  if (Deno.build.os === "darwin") {
    assertStrictEquals(map.get(-48)?.[0], "EADDRINUSE");
  } else {
    assertStrictEquals(map.get(eaddrinuse)?.[0], "EADDRINUSE");
  }
});

Deno.test("[util] transferableAbortController() returns an AbortController", () => {
  const ac = util.transferableAbortController();
  assert(ac instanceof AbortController);
  assert(ac.signal instanceof AbortSignal);
  assertEquals(ac.signal.aborted, false);
  ac.abort("reason");
  assertEquals(ac.signal.aborted, true);
  assertEquals(ac.signal.reason, "reason");
});

Deno.test("[util] transferableAbortSignal() returns the given signal", () => {
  const ac = new AbortController();
  assertStrictEquals(util.transferableAbortSignal(ac.signal), ac.signal);
  // Also accepts already-aborted signals.
  const aborted = AbortSignal.abort("boom");
  assertStrictEquals(util.transferableAbortSignal(aborted), aborted);
});

Deno.test("[util] transferableAbortSignal() throws on non-AbortSignal", () => {
  // deno-lint-ignore no-explicit-any
  const fn = util.transferableAbortSignal as (signal: any) => unknown;
  assertThrows(() => fn(undefined), TypeError);
  assertThrows(() => fn(null), TypeError);
  assertThrows(() => fn({}), TypeError);
  assertThrows(() => fn("signal"), TypeError);
});
