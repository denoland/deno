// Bind an OS-assigned port so that concurrently running tests can never
// collide on a fixed port; `onListen` reports the port that was assigned.
const { promise: listening, resolve: onListening } = Promise.withResolvers<
  number
>();

(async () => {
  const port = await listening;
  const response = await fetch(`http://localhost:${port}/`);
  console.log(await response.text());
  Deno.exit(0);
})();

export default {
  onListen(addr) {
    if (addr.transport !== "tcp") {
      throw new Error(`expected a tcp listener, got ${addr.transport}`);
    }
    // The port is assigned by the OS, so it can't be part of the expected
    // output.
    console.log("listening");
    onListening(addr.port);
  },
  fetch(_req) {
    return new Response("Hello world!");
  },
} satisfies Deno.ServeDefaultExport;
