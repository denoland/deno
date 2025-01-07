// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file

const foo = "baz123";

/* ✓ GOOD */
Vue.component("todo-item", {
  // ...
});

/* ✗ BAD */
Vue.component("Todo", {
  // ...
});

describe("foo", () => {});
describe("foo", () => {});
