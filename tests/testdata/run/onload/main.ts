// deno-lint-ignore-file no-window-prefix no-prototype-builtins
import { assert } from "../../../../../test_util/std/assert/mod.ts";
import "./imported.ts";

assert(window.hasOwnProperty("onload"));
assert(window.onload === null);

const eventHandler = (e: Event) => {
  assert(e.type === "beforeunload" ? e.cancelable : !e.cancelable);
  console.log(`got ${e.type} event in event handler (main)`);
};

window.addEventListener("load", eventHandler);

window.addEventListener("beforeunload", eventHandler);

window.addEventListener("unload", eventHandler);

window.onload = (e: Event) => {
  assert(!e.cancelable);
  console.log(`got ${e.type} event in onload function`);
};

window.onbeforeunload = (e: BeforeUnloadEvent) => {
  assert(e.cancelable);
  console.log(`got ${e.type} event in onbeforeunload function`);
};

window.onunload = (e: Event) => {
  assert(!e.cancelable);
  console.log(`got ${e.type} event in onunload function`);
};

console.log("log from main");
