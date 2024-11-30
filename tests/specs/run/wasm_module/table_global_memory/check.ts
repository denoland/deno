import { memory, table } from "./mod.wasm";
const value1: number = table.get(0);
const value2: number = memory.buffer;
console.log(value1, value2);
