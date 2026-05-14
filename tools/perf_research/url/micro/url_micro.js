// Microbench for URL / URLSearchParams.
//
// Run with each runtime:
//   deno run -A --no-prompt micro/url_micro.js
//   node micro/url_micro.js
//   bun micro/url_micro.js

const ITERS = 200_000;

function bench(name, fn) {
  for (let i = 0; i < 1000; i++) fn(i);
  const t0 = performance.now();
  for (let i = 0; i < ITERS; i++) fn(i);
  const t1 = performance.now();
  const ms = t1 - t0;
  const nsPerOp = (ms * 1e6) / ITERS;
  console.log(JSON.stringify({ name, ms: ms.toFixed(2), ns_per_op: nsPerOp.toFixed(1) }));
}

// 1: Simple URL parse
bench("url_construct_simple", () => {
  new URL("https://example.com/path?x=1#y");
});

// 2: URL parse with base
bench("url_construct_with_base", () => {
  new URL("/path?x=1", "https://example.com/base/");
});

// 3: URL parse complex query string
bench("url_construct_query", () => {
  new URL(
    "https://example.com/api/v1/items?sort=name&page=2&filter=active&limit=50&user=alice",
  );
});

// 4: URL.canParse (no allocation)
bench("url_canparse_simple", () => {
  URL.canParse("https://example.com/path");
});

// 5: Access href on parsed URL
const u1 = new URL("https://user:pw@example.com:8443/path/segments?a=1&b=2#frag");
bench("url_get_href", () => {
  u1.href;
});

// 6: Access pathname
bench("url_get_pathname", () => {
  u1.pathname;
});

// 7: Access search
bench("url_get_search", () => {
  u1.search;
});

// 8: Set pathname (triggers reparse)
bench("url_set_pathname", () => {
  u1.pathname = "/new/path";
});

// 9: Set search
bench("url_set_search", () => {
  u1.search = "?new=query";
});

// 10: Get searchParams (each call lazily attaches)
const u2 = new URL("https://example.com/?a=1&b=2&c=3");
bench("url_searchparams_get", () => {
  u2.searchParams.get("a");
});

// 11: URLSearchParams construct from string
bench("usp_construct_string", () => {
  new URLSearchParams("a=1&b=2&c=3&d=4&e=5&f=6&g=7&h=8");
});

// 12: URLSearchParams construct from object
const initObj = { a: "1", b: "2", c: "3", d: "4", e: "5", f: "6" };
bench("usp_construct_obj", () => {
  new URLSearchParams(initObj);
});

// 13: URLSearchParams get
const usp = new URLSearchParams("a=1&b=2&c=3&d=4&e=5&f=6&g=7&h=8");
bench("usp_get", () => {
  usp.get("c");
});

// 14: URLSearchParams toString
bench("usp_tostring", () => {
  usp.toString();
});
