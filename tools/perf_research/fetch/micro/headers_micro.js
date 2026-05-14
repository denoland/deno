// Microbench: Headers ops on a realistic header set (~12 entries).
// Tests: construct, get (hit), get (miss), append, iterate, has, delete.
//
// Run with each runtime:
//   deno run -A micro/headers_micro.js
//   node micro/headers_micro.js
//   bun micro/headers_micro.js

const ITERS = 1_000_000;

function now() {
  return performance.now();
}

const initObj = {
  "Host": "example.com",
  "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
  "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
  "Accept-Language": "en-US,en;q=0.5",
  "Accept-Encoding": "gzip, deflate, br",
  "Content-Type": "application/json",
  "Content-Length": "42",
  "Connection": "keep-alive",
  "Cookie":
    "session=abc; theme=dark; lang=en; cart=1234; ref=https%3A%2F%2Fexample.com",
  "Cache-Control": "no-cache",
  "Origin": "https://example.com",
  "Referer": "https://example.com/page",
};

function bench(name, fn) {
  // warmup
  for (let i = 0; i < 1000; i++) fn(i);
  const t0 = now();
  for (let i = 0; i < ITERS; i++) fn(i);
  const t1 = now();
  const ms = t1 - t0;
  const nsPerOp = (ms * 1e6) / ITERS;
  console.log(JSON.stringify({ name, ms: ms.toFixed(2), ns_per_op: nsPerOp.toFixed(1) }));
}

// 1: Construct from object literal
bench("headers_construct_obj", () => {
  new Headers(initObj);
});

// 2: Construct from entries array
const initArr = Object.entries(initObj);
bench("headers_construct_arr", () => {
  new Headers(initArr);
});

// 3: get (case-mismatch hit) on a pre-built Headers
const builtHeaders = new Headers(initObj);
bench("headers_get_hit", () => {
  builtHeaders.get("content-type");
});

// 4: get miss
bench("headers_get_miss", () => {
  builtHeaders.get("x-not-present");
});

// 5: set on fresh
bench("headers_set_fresh", () => {
  const h = new Headers();
  h.set("content-type", "application/json");
});

// 6: append on fresh
bench("headers_append_fresh", () => {
  const h = new Headers();
  h.append("set-cookie", "a=1");
  h.append("set-cookie", "b=2");
});

// 7: iterate via for..of
bench("headers_iter", () => {
  let s = 0;
  for (const [k, v] of builtHeaders) s += k.length + v.length;
  if (s < 0) console.log(s);
});

// 8: has hit
bench("headers_has_hit", () => {
  builtHeaders.has("content-type");
});

// 9: has miss
bench("headers_has_miss", () => {
  builtHeaders.has("x-not-present");
});
