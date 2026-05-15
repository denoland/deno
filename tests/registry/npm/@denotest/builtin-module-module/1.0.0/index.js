import m1 from "node:module";
import m2 from "module";
const m3 = await import("module");

console.log(typeof m1.Module);
console.log(typeof m2.Module);
console.log(typeof m3.default.Module);
console.log(typeof m1);
console.log(m1 === m1.Module);
