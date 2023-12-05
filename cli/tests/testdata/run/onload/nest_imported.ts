// deno-lint-ignore-file no-window-prefix
<<<<<<< HEAD
import { assert } from "../../../../../test_util/std/assert/mod.ts";
=======
import { assert } from "../../../../../test_util/std/testing/asserts.ts";
>>>>>>> 172e5f0a0 (1.38.5 (#21469))

const handler = (e: Event) => {
  assert(e.type === "beforeunload" ? e.cancelable : !e.cancelable);
  console.log(`got ${e.type} event in event handler (nest_imported)`);
};

window.addEventListener("load", handler);
window.addEventListener("beforeunload", handler);
window.addEventListener("unload", handler);
console.log("log from nest_imported script");
