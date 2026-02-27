import { sleep } from "./lib.mjs";
export { add } from "./lib.mjs";

console.log("import_inner.js before");

await sleep(100);

console.log("import_inner.js after");
