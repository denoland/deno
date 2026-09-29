import { guarded } from "./mod.js";

Deno.test("takes the consequent, never the alternative", () => {
  guarded(1);
});
