// deno-lint-ignore-file no-window-prefix
import { assert } from "../../../../../test_util/std/assert/mod.ts";
import "./nest_imported.ts";

const handler = (e: Event) => {
  assert(e.type === "beforeunload" ? e.cancelable : !e.cancelable);
  console.log(`got ${e.type} event in event handler (imported)`);
};

window.addEventListener("load", handler);
window.addEventListener("beforeunload", handler);
window.addEventListener("unload", handler);
console.log("log from imported script");
