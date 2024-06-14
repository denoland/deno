const data1 = await import("./data.json", { with: { type: "json" } });
// deno-lint-ignore no-import-assertions
const data2 = await import("./data.json", { assert: { type: "json" } });

console.log(data1);
console.log(data2);
