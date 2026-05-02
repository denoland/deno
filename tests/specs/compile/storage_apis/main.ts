localStorage.setItem("a", "A");
console.log(localStorage.getItem("a"));

const cache = await caches.open("v1");
await cache.put(
  new Request("https://example.com/b"),
  new Response("B"),
);
const res = await cache.match(new Request("https://example.com/b"));
if (!res) throw new Error("unreachable: cache entry was just set");
console.log(await res.text());
