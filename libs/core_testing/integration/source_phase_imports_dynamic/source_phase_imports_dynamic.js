// Copyright 2018-2025 the Deno authors. MIT license.
const foo = await import.source("../wasm_imports/add.wasm");
const instance = await WebAssembly.instantiate(foo, {
  "./import_from_wasm.mjs": { add: (a, b) => a + b },
});
console.log(`exported_add: ${instance.exports["exported_add"]()}`);

const bar = await import.source("../wasm_imports/./add.wasm");
console.log(`Aliased duplicate import is equal: ${foo === bar}`);
