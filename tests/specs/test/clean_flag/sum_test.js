import { sum } from "./sum.js";
import { assertEquals } from "@std/assert/equals";

Deno.test("sum()", () => {
  assertEquals(sum(1, 2), 3);
});
