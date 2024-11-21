import * as nodeBuffer from "node:buffer";
import * as nodeTimers from "node:timers";

console.log(`global: ${globalThis === global}`);
console.log(`Buffer: ${Buffer.from}`);
console.log(`setImmediate: ${setImmediate === nodeTimers.setImmediate}`);
console.log(`clearImmediate: ${clearImmediate === nodeTimers.clearImmediate}`);
