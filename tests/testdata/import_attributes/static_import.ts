import data1 from "./data.json" with { type: "json" };
// deno-lint-ignore no-import-assertions
import data2 from "./data.json" assert { type: "json" };

console.log(data1);
console.log(data2);
