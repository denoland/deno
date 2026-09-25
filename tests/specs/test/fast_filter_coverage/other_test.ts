import { coveredByTopLevel } from "./lib_b.ts";

// Executed at top level, so lib_b.ts shows up in the coverage report even
// though every test in this file is filtered out.
coveredByTopLevel();

Deno.test("other", () => {});
