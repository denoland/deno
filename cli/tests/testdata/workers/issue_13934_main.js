// main.js
new Worker(
  new URL("./issue_13934_worker_1.js", import.meta.url).href,
  { type: "module" },
);
