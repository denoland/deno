import { value } from "./mod.ts";

Deno.test("b", () => {
  if (value() !== 42) throw new Error("fail");
});
