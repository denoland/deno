import { assertEquals } from "jsr:@std/assert";
import { main } from "../mod.ts";

Deno.test("main function", () => {
  assertEquals(main(), "helper result");
});
