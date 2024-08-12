import inspector from "node:inspector";
import { assertEquals } from "@std/assert/equals";

Deno.test("[node/inspector] - importing inspector works", () => {
  assertEquals(typeof inspector.open, "function");
});
