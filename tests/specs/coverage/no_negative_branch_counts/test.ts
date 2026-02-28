import { loopWithBranch } from "./mod.ts";

Deno.test("loopWithBranch", () => {
  loopWithBranch([1, -2, 3, -4, 5]);
});
