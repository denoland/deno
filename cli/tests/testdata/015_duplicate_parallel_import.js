// Importing the same module in parallel, the module should only be
// instantiated once.

const promises = new Array(100)
  .fill(null)
  .map(() => import("./subdir/mod1.ts"));

Promise.all(promises).then((imports) => {
  const mod = imports.reduce((first, cur) => {
    if (typeof first !== "object") {
      throw new Error("Expected an object.");
    }
    if (first !== cur) {
      throw new Error("More than one instance of the same module.");
    }
    return first;
  });

  mod.printHello3();
});
