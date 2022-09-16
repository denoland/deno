console.log(await globalThis.caches.delete("v1"));
console.log(await globalThis.caches.open("v1"));
console.log(await globalThis.caches.has("v1"));
console.log(await globalThis.caches.delete("v1"));

const cache = await globalThis.caches.open("v1");
await cache.put("https://deno.com", new Response("deno.com"));
const res = (await cache.match("https://deno.com"));
console.log(await res?.text());
