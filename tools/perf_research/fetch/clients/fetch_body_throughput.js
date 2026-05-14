// Throughput benchmark: fetch GET a body of N MB and consume via .arrayBuffer().
// Measures the end-to-end body-consumption path: socket -> hyper -> JS Response
// -> bytes(). Run against the matching server (which streams a fixed buffer).
//
// Usage:
//   <runtime> clients/fetch_body_throughput.js --url=http://127.0.0.1:8080/bigbody --iters=200
const argv = (typeof Deno !== "undefined" ? Deno.args : process.argv.slice(2));
const args = Object.fromEntries(
  argv.map((a) => a.replace(/^--/, "").split("=", 2)),
);
const url = args.url;
const iters = Number(args.iters ?? "200");

let totalBytes = 0;
const t0 = performance.now();
for (let i = 0; i < iters; i++) {
  const r = await fetch(url);
  const buf = await r.arrayBuffer();
  totalBytes += buf.byteLength;
}
const dt = (performance.now() - t0) / 1000;
console.log(JSON.stringify({
  runtime: typeof Deno !== "undefined"
    ? "deno"
    : (typeof Bun !== "undefined" ? "bun" : "node"),
  url,
  iters,
  duration: dt.toFixed(3),
  total_bytes: totalBytes,
  mb_per_s: ((totalBytes / dt) / 1e6).toFixed(2),
}));
