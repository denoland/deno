// main.js
new Worker(
  new URL("./worker1.js", import.meta.url).href,
  { type: "module" },
);
