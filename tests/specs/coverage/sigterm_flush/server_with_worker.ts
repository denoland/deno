// A long-running server that spawns a web worker on startup.
const worker = new Worker(import.meta.resolve("./worker.ts"), {
  type: "module",
});

// Wait for worker to finish before starting the server.
await new Promise<void>((resolve) => {
  worker.onmessage = () => {
    worker.terminate();
    resolve();
  };
});

Deno.serve({ port: 0 }, (_req: Request) => {
  return new Response("Hello");
});
