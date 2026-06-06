import defer * as deferred from "./deferred.js";

console.log("before access");
console.log(`value: ${deferred.value}`);
console.log("after first access");
console.log(`add: ${deferred.add(1, 2)}`);
console.log("done");
