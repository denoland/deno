import {
  add,
  subtract,
} from "http://localhost:4545/wasm/math_with_import.wasm";

console.log(add(1, 2));
console.log(subtract(100, 50));
