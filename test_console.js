console.group();
console.log("Log from Deno");
console.warn("Warn from Deno");
console.error("Error from Deno");
console.groupEnd();
console.table({ a: 1, b: 2, c: 3});
console.time();
console.timeEnd();
console.trace();
console.assert(1 == 2);

// keep process alive
setInterval(() => {
}, 3000);
