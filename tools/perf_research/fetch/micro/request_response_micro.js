// Microbench: Request and Response object construction and body consumption.
// These objects are allocated on every HTTP request handled by Deno.serve.

const ITERS = 200_000;

function now() {
  return performance.now();
}

function bench(name, fn) {
  for (let i = 0; i < 1000; i++) fn(i);
  const t0 = now();
  for (let i = 0; i < ITERS; i++) fn(i);
  const t1 = now();
  const ms = t1 - t0;
  const nsPerOp = (ms * 1e6) / ITERS;
  console.log(JSON.stringify({ name, ms: ms.toFixed(2), ns_per_op: nsPerOp.toFixed(1) }));
}

// 1: Construct Response from string
bench("response_construct_string", () => {
  new Response("hello world");
});

// 2: Construct Response from Uint8Array
const u8 = new TextEncoder().encode("hello world");
bench("response_construct_u8", () => {
  new Response(u8);
});

// 3: Construct Response with init headers (object literal)
bench("response_construct_with_headers", () => {
  new Response("hello", { headers: { "content-type": "text/plain" } });
});

// 4: Construct Response with init headers (Headers instance)
const reusedHeaders = new Headers({ "content-type": "text/plain" });
bench("response_construct_reused_headers", () => {
  new Response("hello", { headers: reusedHeaders });
});

// 5: Construct Request
bench("request_construct", () => {
  new Request("https://example.com/path?q=1", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: '{"a":1}',
  });
});

// 6: Read body via text() — full pipeline
const bodyBytes = new TextEncoder().encode(
  JSON.stringify({ users: Array.from({ length: 10 }, (_, i) => ({ id: i, name: `u${i}` })) }),
);
const ITERS_BODY = 50_000;
async function benchAsync(name, fn) {
  for (let i = 0; i < 100; i++) await fn(i);
  const t0 = now();
  for (let i = 0; i < ITERS_BODY; i++) await fn(i);
  const t1 = now();
  const ms = t1 - t0;
  const nsPerOp = (ms * 1e6) / ITERS_BODY;
  console.log(JSON.stringify({ name, ms: ms.toFixed(2), ns_per_op: nsPerOp.toFixed(1) }));
}

await benchAsync("response_text_smallish", async () => {
  const r = new Response(bodyBytes);
  await r.text();
});

await benchAsync("response_json_smallish", async () => {
  const r = new Response(bodyBytes);
  await r.json();
});
