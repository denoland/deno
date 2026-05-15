import { f } from "./mod.ts";

Deno.test("f", () => {
  f(true);
  f(false);
});
