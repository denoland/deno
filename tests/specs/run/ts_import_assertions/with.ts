import test from "./data.json" with { type: "json" };
console.log(test);
console.log((await import("./data.json", { with: { type: "json" } })).default);
