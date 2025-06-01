import { subtract } from "./mod.ts";
import { assertEquals } from "@std/assert";

Deno.test("subtract", () => {
  assertEquals(subtract(4, 2), 2);
});
