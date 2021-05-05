import { assert } from "../../../test_util/std/testing/asserts.ts";
import "./nest_imported.ts";

const handler = (e: Event): void => {
  assert(!e.cancelable);
  console.log(`got ${e.type} event in event handler (imported)`);
};

window.addEventListener("load", handler);
window.addEventListener("unload", handler);
console.log("log from imported script");
