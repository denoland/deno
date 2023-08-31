import { assertEquals } from "{CURRENT_STD_URL}assert/mod.ts";
import { add } from "./main.ts";

Deno.test(function addTest() {
  assertEquals(add(2, 3), 5);
});
