import source mathSource from "./math.wasm";
const mathInstance = await WebAssembly.instantiate(mathSource);
console.log(`add(1, 2): ${mathInstance.exports["add"](1, 2)}`);
console.log(`subtract(8, 2): ${mathInstance.exports["subtract"](8, 2)}`);

import source mathSource2 from "././math.wasm";
console.log(`Aliased duplicate import is equal: ${mathSource === mathSource2}`);
