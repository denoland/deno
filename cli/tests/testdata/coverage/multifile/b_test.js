import { test } from "./mod.js";

Deno.test({
  name: "bugrepo b",
  fn: () => {
    test(false);
  },
});
