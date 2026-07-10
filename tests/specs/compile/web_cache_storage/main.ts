// The Web `caches` API should be available in a compiled binary (no
// `--location` required) instead of throwing `NotSupported`.
const cache = await caches.open("v1");
await cache.put(
  new Request("https://example.com/a"),
  new Response("cached body"),
);
const res = await cache.match(new Request("https://example.com/a"));
if (!res) {
  throw new Error("cache miss: entry was just inserted");
}
console.log(await res.text());
