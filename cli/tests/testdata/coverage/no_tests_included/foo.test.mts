import { addNumbers } from "./foo.ts";
<<<<<<< HEAD
import { assertEquals } from "../../../../../test_util/std/assert/mod.ts";
=======
import { assertEquals } from "../../../../../test_util/std/testing/asserts.ts";
>>>>>>> 172e5f0a0 (1.38.5 (#21469))

Deno.test("addNumbers works", () => {
  assertEquals(addNumbers(1, 2), 3);
});
