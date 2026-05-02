import { add } from "./source.ts";

Deno.test("add()", () => {
  if (add(1, 2) !== 3) {
    throw new Error("test failed");
  }
});
