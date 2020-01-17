import { add, addImported, addRemote } from "./051_wasm_import/simple.wasm";
import { state } from "./051_wasm_import/wasm-dep.js";

function assertEquals(actual: unknown, expected: unknown, msg?: string): void {
  if (actual !== expected) {
    throw new Error(msg);
  }
}

assertEquals(state, "WASM Start Executed", "Incorrect state");

assertEquals(add(10, 20), 30, "Incorrect add");

assertEquals(addImported(0), 42, "Incorrect addImported");

assertEquals(state, "WASM JS Function Executed", "Incorrect state");

assertEquals(addImported(1), 43, "Incorrect addImported");

assertEquals(addRemote(1), 2020, "Incorrect addRemote");

console.log("Passed");
