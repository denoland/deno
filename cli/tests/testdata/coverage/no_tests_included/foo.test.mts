import { addNumbers } from './foo.ts';
import { assertEquals } from "../../../../../test_util/std/testing/asserts.ts";

Deno.test("addNumbers works", () => {
  assertEquals(addNumbers(1, 2), 3);
});
