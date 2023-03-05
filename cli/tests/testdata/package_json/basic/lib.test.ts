import { add } from "./lib.ts";

Deno.test("should add", () => {
  if (add(1, 2) !== 3) {
    throw new Error("Fail");
  }
});
