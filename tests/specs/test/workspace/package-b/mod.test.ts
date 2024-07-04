import { addOne } from "./mod.ts";

Deno.test("addOne", () => {
  if (addOne(1) !== 2) {
    throw new Error("failed");
  }
});

Deno.test("fail", () => {
  throw new Error("failed");
});
