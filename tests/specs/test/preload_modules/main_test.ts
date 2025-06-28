Deno.test("hello", () => {
  // @ts-ignore These are provided by the preload scripts, but we don't provide typings for them
  console.log("Value of __preload__", globalThis.__preload__);
  // @ts-ignore These are provided by the preload scripts, but we don't provide typings for them
  console.log("Value of __import__", globalThis.__import__);
});
