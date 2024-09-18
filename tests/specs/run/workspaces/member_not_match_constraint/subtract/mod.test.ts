import { subtract } from "./mod.ts";

Deno.test("subtract", () => {
  if (subtract(3, 2) !== 1) {
    throw new Error("fail");
  }
});
