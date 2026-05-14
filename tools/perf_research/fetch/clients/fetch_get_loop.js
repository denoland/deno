// fetch() client: hit a local server URL N times with K concurrency, report
// requests/second over a fixed duration. Portable across Deno/Node/Bun.
//
// Usage:
//   deno run -A clients/fetch_get_loop.js --url=http://127.0.0.1:8080/hello --duration=5 --concurrency=64
//   node clients/fetch_get_loop.js --url=http://127.0.0.1:8081/hello --duration=5 --concurrency=64
//   bun clients/fetch_get_loop.js --url=http://127.0.0.1:8082/hello --duration=5 --concurrency=64

const argv = (typeof Deno !== "undefined" ? Deno.args : process.argv.slice(2));
const args = Object.fromEntries(
  argv.map((a) => a.replace(/^--/, "").split("=", 2)),
);
const url = args.url;
const duration = Number(args.duration ?? "5");
const concurrency = Number(args.concurrency ?? "64");
const consumeBody = args.body !== "skip";

const endAt = Date.now() + duration * 1000;
let done = 0;
let bytes = 0;
let errors = 0;

async function worker() {
  while (Date.now() < endAt) {
    try {
      const r = await fetch(url);
      if (consumeBody) {
        const b = await r.arrayBuffer();
        bytes += b.byteLength;
      } else {
        await r.body?.cancel();
      }
      done++;
    } catch {
      errors++;
    }
  }
}

const t0 = performance.now();
await Promise.all(Array.from({ length: concurrency }, () => worker()));
const dt = (performance.now() - t0) / 1000;

console.log(JSON.stringify({
  runtime: typeof Deno !== "undefined"
    ? "deno"
    : (typeof Bun !== "undefined" ? "bun" : "node"),
  url,
  duration: dt.toFixed(3),
  concurrency,
  requests: done,
  errors,
  bytes,
  rps: (done / dt).toFixed(1),
  mb_per_s: ((bytes / dt) / 1e6).toFixed(2),
}));
