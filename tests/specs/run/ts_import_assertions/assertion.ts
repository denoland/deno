import test from "./data.json" assert { type: "json" };
console.log(test);
console.log(
  (await import("./data.json", { assert: { type: "json" } })).default,
);
