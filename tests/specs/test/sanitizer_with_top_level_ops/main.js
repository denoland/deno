// An async IIFE that throws off ops every 100ms
(async () => {
  while (true) {
    await new Promise((r) => setTimeout(r, 10));
  }
})();

// An HTTP server that resolves every request
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

// We need to ensure that these ops are balanced at the top-level to avoid triggering
// the sanitizer.
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
  await (await fetch(`http://127.0.0.1:${port}/`)).text();
  await new Promise((r) => setTimeout(r, 100));
});
