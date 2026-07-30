const cache = await caches.open("test");
const response = await cache.match("https://example.com/data");
if (response === undefined) {
  throw new Error("configured cache endpoint returned no response");
}
console.log(await response.text());
