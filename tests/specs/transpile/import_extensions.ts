import "./helpers.ts";
import "./view.jsx";
import helper = require("./helper.cts");
console.log(helper);
export { add } from "./helpers.ts";
export * from "./helper.mts";

await import("./helper.cts");

console.log("./literal.ts");
