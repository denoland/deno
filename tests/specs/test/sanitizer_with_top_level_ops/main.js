// The sanitizers must ignore any ops, resources or timers that are
// "replaced" at the top level with a thing of the same kind.

// An async IIFE that throws off timers every 10ms
(async () => {
  while (true) {
    await new Promise((r) => setTimeout(r, 10));
  }
})();

// An HTTP server that resolves an op for every request
const { promise, resolve } = Promise.withResolvers();
const server = Deno.serve({
  port: 0,
  onListen: ({ port }) => resolve(port),
  handler: () => new Response("ok"),
});
const port = await promise;

// A TCP listener loop
const listener = Deno.listen({ port: 0 });
const conn1 = await Deno.connect({ port: listener.addr.port });
const conn2 = await listener.accept();

// Note: we need to ensure that these read/write ops are balanced at the top-level to avoid triggering
// the sanitizer, so we use two async IIFEs.
(async () => {
  // This will write without blocking for a bit but eventually will start writing async
  // once the tokio coop kicks in or the buffers fill up.
  while (true) {
    await conn1.write(new Uint8Array(1024));
  }
})();
(async () => {
  while (true) {
    await conn2.read(new Uint8Array(10 * 1024));
  }
})();

Deno.test(async function waits() {
  // Trigger the server to restart its op
  await (await fetch(`http://127.0.0.1:${port}/`)).text();
  // Let the IIFEs run for a bit
  await new Promise((r) => setTimeout(r, 100));
});
