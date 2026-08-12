// A Wasm module can import a global from a plain JS module (not another Wasm
// module). The JS dependency is not registered in `import.meta.wasmInstances`,
// so the generated global-import binding falls back to the JS value directly
// (here the number `42`), which satisfies an immutable global import.
import { read } from "./mod.wasm";

console.log("read", read());
