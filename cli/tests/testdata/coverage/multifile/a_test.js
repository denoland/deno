import { test } from "./mod.js";

Deno.test({
  name: "bugrepo a",
  fn: () => {
    test(true);
  },
});
