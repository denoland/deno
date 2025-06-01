const r = await fetch(
  "http://localhost:4545/workers/fetching_worker.js",
);
await r.text();
postMessage("Done!");
close();
