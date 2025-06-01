import { add } from "./mod.ts";

Deno.test("add", () => {
  if (add(1, 2) !== 3) {
    throw new Error("failed");
  }
});
