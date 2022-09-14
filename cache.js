console.log("has('name'):", await caches.has("name"));
console.log("open('name'):", await caches.open("name"));
console.log("has('name'):", await caches.has("name"));
console.log("delete('name'):", await caches.delete("name"));
console.log("has('name'):", await caches.has("name"));
console.log("delete('name'):", await caches.delete("name"));

const req = new Request("https://example.com");
const res = new Response("Response for example.com", {
  headers: {
    "name": "response",
  },
});

const cache = await caches.open("name");
await cache.put(req, res);

const res1 = await cache.match(req);
console.log(res1);
console.log("text", await res1.text());
