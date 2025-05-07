import { assert } from "@std/assert";
import "./nest_imported.ts";

const handler = (e: Event) => {
  assert(e.type === "beforeunload" ? e.cancelable : !e.cancelable);
  console.log(`got ${e.type} event in event handler (imported)`);
};

globalThis.addEventListener("load", handler);
globalThis.addEventListener("beforeunload", handler);
globalThis.addEventListener("unload", handler);
console.log("log from imported script");
