const output = await Deno.readTextFile("dist/import_extensions.js");

export {};

console.log(
  output.includes('import "./helpers.js";') &&
    output.includes('export { add } from "./helpers.js";') &&
    output.includes('export * from "./helper.mjs";') &&
    output.includes('await import("./helper.cjs");') &&
    output.includes('const helper = require("./helper.cjs");') &&
    output.includes('import "./view.js";') &&
    output.includes('console.log("./literal.ts");'),
);
