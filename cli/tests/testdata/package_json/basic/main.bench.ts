import { add } from "./main.ts";

Deno.bench("should add", () => {
  if (add(1, 2) !== 3) {
    throw new Error("Fail");
  }
});
