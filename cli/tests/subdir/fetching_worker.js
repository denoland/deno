const r = await fetch(
  "http://localhost:4545/cli/tests/subdir/fetching_worker.js",
);
await r.text();
postMessage("Done!");
close();
