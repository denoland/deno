import { add } from "./mod.ts";

// this test should not be run or linted
Deno.test("add", () => {
  let unusedVar = 5; // purposefully causing a lint error to ensure it's not linted
  if (add(1, 2) !== 3) {
    throw new Error("fail");
  }
});
