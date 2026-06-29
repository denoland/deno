Deno.test("pending promise waits for worker termination", async () => {
  const worker = new Worker(new URL("./worker.ts", import.meta.url), {
    type: "module",
  });

  await new Promise(() => worker.terminate());
});
