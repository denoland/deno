// deno-lint-ignore-file no-window-prefix
import { assert } from "@std/assert";

const handler = (e: Event) => {
  assert(e.type === "beforeunload" ? e.cancelable : !e.cancelable);
  console.log(`got ${e.type} event in event handler (nest_imported)`);
};

window.addEventListener("load", handler);
window.addEventListener("beforeunload", handler);
window.addEventListener("unload", handler);
console.log("log from nest_imported script");
