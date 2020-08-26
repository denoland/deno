import {
  assert,
  assertEquals,
  assertNotStrictEquals,
  assertStrictEquals,
} from "../testing/asserts.ts";
import { stripColor } from "../fmt/colors.ts";
import { copyError, inspectValue, createErrDiff } from "./assertion_error.ts";

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
