import { add_numbers } from './foo.ts';
import { assertEquals } from "https://deno.land/std@0.183.0/testing/asserts.ts";

Deno.test("add_numbers works", () => {
  assertEquals(add_numbers(1, 2), 3);
});
