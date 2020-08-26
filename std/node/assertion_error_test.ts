import { stripColor } from "../fmt/colors.ts";
import {
  assert,
  assertEquals,
  assertNotStrictEquals,
  assertStrictEquals,
} from "../testing/asserts.ts";
import {
  AssertionError,
  copyError,
  inspectValue,
  createErrDiff,
} from "./assertion_error.ts";

Deno.test({
  name: "copyError()",
  fn() {
    class TestError extends Error {}
    const err = new TestError("this is a test");
    const copy = copyError(err);

    assert(copy instanceof Error, "Copy should inherit from Error.");
    assert(copy instanceof TestError, "Copy should inherit from TestError.");
    assertEquals(copy, err, "Copy should be equal to the original error.");
    assertNotStrictEquals(
      copy,
      err,
      "Copy should not be strictly equal to the original error.",
    );
  },
});

Deno.test({
  name: "inspectValue()",
  fn() {
    const obj = { a: 1, b: [2] };
    Object.defineProperty(obj, "c", { value: 3, enumerable: false });
    assertStrictEquals(
      stripColor(inspectValue(obj)),
      "{ a: 1, b: [ 2 ] }",
    );
  },
});

Deno.test({
  name: "createErrDiff()",
  fn() {
    assertStrictEquals(
      stripColor(
        createErrDiff({ a: 1, b: 2 }, { a: 2, b: 2 }, "strictEqual"),
      ),
      stripColor(
        'Expected "actual" to be reference-equal to "expected":' + "\n" +
          "+ actual - expected" + "\n" +
          "\n" +
          "+ { a: 1, b: 2 }" + "\n" +
          "- { a: 2, b: 2 }",
      ),
    );
  },
});

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
    assertStrictEquals(stripColor(err.message), "1 equal 2");
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
    assertStrictEquals(stripColor(err.message), "deno match /node/");
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
