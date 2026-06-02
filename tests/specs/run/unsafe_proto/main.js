// The `__proto__` accessor is disabled by default. Reading it throws a
// descriptive TypeError instead of silently returning `undefined`.
try {
  ({}).__proto__;
  console.log("did not throw");
} catch (e) {
  console.log(e instanceof TypeError);
  console.log(e.message);
}

new Worker(import.meta.resolve("./worker.js"), {
  type: "module",
});
