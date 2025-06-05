console.log("Value of __preload__", globalThis.__preload__);

console.log("Value of __import__", globalThis.__import__);

Deno.bench("hello", () => {
});
