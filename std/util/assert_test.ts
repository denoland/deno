import { assert, AssertionError } from "./assert.ts";
import { assertThrows } from "../testing/asserts.ts";

Deno.test({
  name: "[util/assert]",
  fn() {
    const truePatterns = [1, "1", true, Symbol("1"), {}, [], new Date()];
    for (const pat of truePatterns) {
      assert(pat);
    }
    const falsePatterns = [0, "", false, null, undefined];
    for (const pat of falsePatterns) {
      assertThrows(() => assert(pat), AssertionError);
    }
  },
});
