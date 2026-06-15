// With --unsafe-proto the native accessor is restored, so reading
// `__proto__` returns the prototype and this prints `true`.
console.log(({}).__proto__ !== undefined);

new Worker(import.meta.resolve("./worker.js"), {
  type: "module",
});
