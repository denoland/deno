// Copyright 2018-2025 the Deno authors. MIT license.
import { stripAnsiCode } from "@std/fmt/colors";
import { assert, assertStrictEquals } from "@std/assert";
import { AssertionError } from "node:assert";

Deno.test({
  name: "construct AssertionError() with given message",
  fn() {
    const err = new AssertionError(
      {
        message: "answer",
        actual: "42",
        expected: "42",
        operator: "notStrictEqual",
      },
    );
    assertStrictEquals(err.name, "AssertionError");
    assertStrictEquals(err.message, "answer");
    assertStrictEquals(err.generatedMessage, false);
    assertStrictEquals(err.code, "ERR_ASSERTION");
    assertStrictEquals(err.actual, "42");
    assertStrictEquals(err.expected, "42");
    assertStrictEquals(err.operator, "notStrictEqual");
  },
});

Deno.test({
  name: "construct AssertionError() with generated message",
  fn() {
    const err = new AssertionError(
      { actual: 1, expected: 2, operator: "equal" },
    );
    assertStrictEquals(err.name, "AssertionError");
    assertStrictEquals(stripAnsiCode(err.message), "1 equal 2");
    assertStrictEquals(err.generatedMessage, true);
    assertStrictEquals(err.code, "ERR_ASSERTION");
    assertStrictEquals(err.actual, 1);
    assertStrictEquals(err.expected, 2);
    assertStrictEquals(err.operator, "equal");
  },
});

Deno.test({
  name: "construct AssertionError() with stackStartFn",
  fn: function stackStartFn() {
    const expected = /node/;
    const err = new AssertionError({
      actual: "deno",
      expected,
      operator: "match",
      stackStartFn,
    });
    assertStrictEquals(err.name, "AssertionError");
    assertStrictEquals(stripAnsiCode(err.message), `'deno' match /node/`);
    assertStrictEquals(err.generatedMessage, true);
    assertStrictEquals(err.code, "ERR_ASSERTION");
    assertStrictEquals(err.actual, "deno");
    assertStrictEquals(err.expected, expected);
    assertStrictEquals(err.operator, "match");
    assert(err.stack, "error should have a stack");
    assert(
      !err.stack?.includes("stackStartFn"),
      "stackStartFn() should not present in stack trace",
    );
  },
});
