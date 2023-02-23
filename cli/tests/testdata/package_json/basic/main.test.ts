import { add } from "./main.ts";

Deno.test("should add", () => {
  if (add(1, 2) !== 3) {
    throw new Error("Fail");
  }
});
