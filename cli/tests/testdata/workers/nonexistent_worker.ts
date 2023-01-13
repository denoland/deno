const w = new Worker(import.meta.resolve("./doesnt_exist.js"), {
  type: "module",
});

w.postMessage("hello");
