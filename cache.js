console.log("has('name'):", await caches.has("name"));
console.log("open('name'):", await caches.open("name"));
console.log("has('name'):", await caches.has("name"));
console.log("delete('name'):", await caches.delete("name"));
console.log("has('name'):", await caches.has("name"));
console.log("delete('name'):", await caches.delete("name"));

const req = new Request("https://example.com");
const res = new Response("Response for example.com");

const cache = await caches.open("name");
await cache.put(req, res);
