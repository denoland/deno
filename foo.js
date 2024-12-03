const foo = "baz123";

/* ✓ GOOD */
Vue.component("todo-item", {
  // ...
});

/* ✗ BAD */
Vue.component("Todo", {
  // ...
});
