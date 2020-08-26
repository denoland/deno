import {
  assert,
  assertEquals,
  assertNotStrictEquals,
  assertStrictEquals,
} from "../testing/asserts.ts";
import { stripColor } from "../fmt/colors.ts";
import { copyError, inspectValue } from "./assertion_error.ts";

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
    console.log();
    const obj = { a: 1, b: [2] };
    Object.defineProperty(obj, "c", { value: 3, enumerable: false });
    assertStrictEquals(
      stripColor(inspectValue(obj)),
      "{\n  a: 1,\n  b: [\n    2\n  ]\n}",
    );
  },
});
