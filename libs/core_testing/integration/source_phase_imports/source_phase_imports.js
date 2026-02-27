// Copyright 2018-2025 the Deno authors. MIT license.
import source foo from "../wasm_imports/add.wasm";
const instance = await WebAssembly.instantiate(foo, {
  "./import_from_wasm.mjs": { add: (a, b) => a + b },
});
console.log(`exported_add: ${instance.exports["exported_add"]()}`);

import source bar from "../wasm_imports/./add.wasm";
console.log(`Aliased duplicate import is equal: ${foo === bar}`);
