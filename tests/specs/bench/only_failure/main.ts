import { assert } from "@std/assert";

Deno.bench({
  name: "before",
  fn() {},
});

Deno.bench({
  only: true,
  name: "only",
  fn() {
    assert(false);
  },
});

Deno.bench({
  name: "after",
  fn() {},
});
