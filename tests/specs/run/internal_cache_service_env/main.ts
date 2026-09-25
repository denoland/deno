if (Deno.env.get("DENO_WEBGPU_TRACE") !== Deno.args[0]) {
  throw new Error("configured WebGPU trace directory was not preserved");
}

const cache = await caches.open("test");
const response = await cache.match("https://example.com/data");
if (response === undefined) {
  throw new Error("configured cache endpoint returned no response");
}
console.log(await response.text());
