import { addNumbers } from "./foo.ts";
import { assertEquals } from "../../../../tests/util/std/assert/mod.ts";

Deno.test("addNumbers works", () => {
  assertEquals(addNumbers(1, 2), 3);
});
