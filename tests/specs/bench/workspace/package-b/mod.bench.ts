import { addOne } from "./mod.ts";

Deno.bench("addOne", () => {
  if (addOne(1) !== 2) {
    throw new Error("failed");
  }
});
