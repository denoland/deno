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
const listener = Deno.listen({ port: 8080 });
const conn1 = await Deno.connect({ port: 8080 });
const conn2 = await listener.accept();

(async () => {
  while (true) {
    await conn1.write(new TextEncoder().encode("Hello World"));
    await conn2.read(new Uint8Array(11));
    await new Promise((r) => setTimeout(r, 1));
  }
})();

Deno.test(async function waits() {
  await (await fetch(`http://127.0.0.1:${port}/`)).text();
  await new Promise((r) => setTimeout(r, 100));
});
