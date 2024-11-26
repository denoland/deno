import { randomTimeout } from "./util.js";

console.log("Starting build1");

await randomTimeout(500, 750);
console.log("build1 performing more work...");
await randomTimeout(500, 750);

console.log("build1 finished");
