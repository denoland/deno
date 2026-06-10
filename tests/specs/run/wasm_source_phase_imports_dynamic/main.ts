const mathSource = await import.source("./math.wasm");
const mathInstance = await WebAssembly.instantiate(mathSource);
console.log(`add(1, 2): ${mathInstance.exports["add"](1, 2)}`);
console.log(`subtract(8, 2): ${mathInstance.exports["subtract"](8, 2)}`);

const mathSource2 = await import.source("././math.wasm");
console.log(`Aliased duplicate import is equal: ${mathSource === mathSource2}`);
