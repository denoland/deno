import { randomTimeout } from "./util.js";

console.log("Starting build2");

await randomTimeout(250, 750);
console.log("build2 performing more work...");
await randomTimeout(250, 750);

console.log("build2 finished");
