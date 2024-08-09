import { assertEquals } from "../../../util/std/assert/mod.ts";

Deno.test("assert a b", () => {
  assertEquals("foo", "bar");
});
