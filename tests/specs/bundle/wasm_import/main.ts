import * as add from "./add.wasm";
import * as calc from "./calc.wasm";

// `import * as` on a `.wasm` module should expose the instance's exports, not
// the raw bytes (see denoland/deno#32104).
console.log("add keys:", Object.keys(add));
console.log("add(2, 3):", add.add(2, 3));

// A `.wasm` module that imports a function from a sibling JS module should have
// that import resolved and bundled too.
console.log("callImport(41):", calc.callImport(41));
