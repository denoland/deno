import { add } from "./mod.ts";

Deno.bench("add", () => {
  if (add(1, 2) !== 3) {
    throw new Error("failed");
  }
});
