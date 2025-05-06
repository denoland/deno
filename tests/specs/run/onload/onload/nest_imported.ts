import { assert } from "@std/assert";

const handler = (e: Event) => {
  assert(e.type === "beforeunload" ? e.cancelable : !e.cancelable);
  console.log(`got ${e.type} event in event handler (nest_imported)`);
};

globalThis.addEventListener("load", handler);
globalThis.addEventListener("beforeunload", handler);
globalThis.addEventListener("unload", handler);
console.log("log from nest_imported script");
