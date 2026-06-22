import { value } from "../b/mod.ts";

Deno.test("a", () => {
  if (value() !== 42) throw new Error("fail");
});
