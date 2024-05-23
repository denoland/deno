import { sum } from "./sum.js";
import { assertEquals } from "../../../util/std/assert/assert_equals.ts";

Deno.test("sum()", () => {
  assertEquals(sum(1, 2), 3);
});
