import { add, addImported, addRemote } from "./051_wasm_import/simple.wasm";
import { state } from "./051_wasm_import/wasm-dep.js";

Deno.assert.equals(state, "WASM Start Executed", "Incorrect state");

Deno.assert.equals(add(10, 20), 30, "Incorrect add");

Deno.assert.equals(addImported(0), 42, "Incorrect addImported");

Deno.assert.equals(state, "WASM JS Function Executed", "Incorrect state");

Deno.assert.equals(addImported(1), 43, "Incorrect addImported");

Deno.assert.equals(addRemote(1), 2020, "Incorrect addRemote");

console.log("Passed");
