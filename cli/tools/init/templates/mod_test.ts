import { assertEquals } from "{CURRENT_STD_URL}testing/asserts.ts";
import { add } from "./mod.ts";

Deno.test(function addTest() {
  assertEquals(add(2, 3), 5);
});
