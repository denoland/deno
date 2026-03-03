import defer * as deferred from "./deferred.js";

console.log("before access");

// First property access triggers module evaluation
console.log(`value: ${deferred.value}`);

console.log("after first access");

// Subsequent accesses use the already-evaluated module
console.log(`add: ${deferred.add(1, 2)}`);

console.log("done");
