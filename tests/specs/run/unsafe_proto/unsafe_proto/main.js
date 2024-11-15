console.log(Object.hasOwn(Object.prototype, "__proto__"));

new Worker(import.meta.resolve("./worker.js"), {
  type: "module",
});
