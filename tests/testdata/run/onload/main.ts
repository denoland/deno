// deno-lint-ignore-file no-prototype-builtins
import { assert } from "@std/assert";
import "./imported.ts";

assert(globalThis.hasOwnProperty("onload"));
assert(globalThis.onload === null);

const eventHandler = (e: Event) => {
  assert(e.type === "beforeunload" ? e.cancelable : !e.cancelable);
  console.log(`got ${e.type} event in event handler (main)`);
};

globalThis.addEventListener("load", eventHandler);

globalThis.addEventListener("beforeunload", eventHandler);

globalThis.addEventListener("unload", eventHandler);

globalThis.onload = (e: Event) => {
  assert(!e.cancelable);
  console.log(`got ${e.type} event in onload function`);
};

globalThis.onbeforeunload = (e: BeforeUnloadEvent) => {
  assert(e.cancelable);
  console.log(`got ${e.type} event in onbeforeunload function`);
};

globalThis.onunload = (e: Event) => {
  assert(!e.cancelable);
  console.log(`got ${e.type} event in onunload function`);
};

console.log("log from main");
