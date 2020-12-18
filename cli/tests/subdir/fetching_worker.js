console.error("before fetch");
const r = await fetch(
  "http://localhost:4545/cli/tests/subdir/fetching_worker.js",
);
console.error("after fetch");
await r.text();
console.error("after text");
postMessage("Done!");
console.error("after postmessage");
close();
console.error("after close");
