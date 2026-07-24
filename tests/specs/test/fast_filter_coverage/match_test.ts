import { coveredByTest } from "./lib_a.ts";

Deno.test("match", () => {
  coveredByTest();
});
