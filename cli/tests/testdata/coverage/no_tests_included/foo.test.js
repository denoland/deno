import { addNumbers } from "./foo.ts";
import { assertEquals } from "https://deno.land/std@0.183.0/testing/asserts.ts";

Deno.test("addNumbers works", () => {
  assertEquals(addNumbers(1, 2), 3);
});
