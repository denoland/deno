console.log("before dynamic import defer");

const deferred = await import.defer("./deferred.js");

console.log("after dynamic import defer, before access");

// First property access triggers module evaluation
console.log(`value: ${deferred.value}`);

console.log("after first access");

console.log(`add: ${deferred.add(1, 2)}`);

console.log("done");
