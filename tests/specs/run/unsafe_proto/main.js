// Disabled by default: the accessor stays installed (so writes can be
// captured) but reads return `undefined`, so this prints `false`.
console.log(({}).__proto__ !== undefined);

new Worker(import.meta.resolve("./worker.js"), {
  type: "module",
});
