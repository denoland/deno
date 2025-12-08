import { assert } from '@std/assert';

Deno.test({
  name: "before",
  fn() {},
});

Deno.test({
  only: true,
  name: "only",
  fn() {
    assert(false);
  },
});

Deno.test.only({
  name: "only2",
  fn() {},
});

Deno.test({
  name: "after",
  fn() {},
});
