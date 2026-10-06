// The handler keeps reading the request body in the background and answers
// with a streamed response that then goes idle. While the body read is parked
// on the socket, the response writer leaves the socket to it, so a client
// disconnect must still reach the writer and cancel the response stream.

const BODY_SIZE = 1024 * 1024;
const SENT = 64 * 1024;

const { promise: cancelled, resolve: resolveCancelled } = Promise
  .withResolvers<void>();
const { promise: bodyParked, resolve: resolveBodyParked } = Promise
  .withResolvers<void>();

const server = Deno.serve(
  { port: 0, hostname: "127.0.0.1", onListen: () => {} },
  async (req) => {
    const reader = req.body!.getReader();
    let read = 0;
    while (read < SENT) {
      const { value, done } = await reader.read();
      if (done) break;
      read += value.byteLength;
    }
    (async () => {
      try {
        // Parks on the socket: the rest of the body is never sent.
        const pending = reader.read();
        resolveBodyParked();
        await pending;
      } catch {
        // The client went away mid-body.
      }
    })();
    return new Response(
      new ReadableStream({
        start(controller) {
          controller.enqueue(new Uint8Array([1]));
        },
        cancel() {
          resolveCancelled();
        },
      }),
    );
  },
);

const timeout = setTimeout(() => {
  console.log("timed out waiting for the response to be cancelled");
  Deno.exit(1);
}, 10_000);

const conn = await Deno.connect({
  port: server.addr.port,
  hostname: "127.0.0.1",
});
await conn.write(
  new TextEncoder().encode(
    `PUT / HTTP/1.1\r\nHost: x\r\nContent-Length: ${BODY_SIZE}\r\n\r\n`,
  ),
);
const body = new Uint8Array(SENT);
let written = 0;
while (written < body.length) {
  written += await conn.write(body.subarray(written));
}

// Wait for the response head and the first chunk.
const buf = new Uint8Array(4096);
let response = "";
while (!response.includes("\r\n\r\n")) {
  const n = await conn.read(buf);
  if (n === null) throw new Error("connection closed early");
  response += new TextDecoder("latin1").decode(buf.subarray(0, n));
}
await bodyParked;
// Let the response writer go idle before disconnecting.
await new Promise((resolve) => setTimeout(resolve, 100));
console.log("response received");

conn.close();
await cancelled;
console.log("response cancelled");
clearTimeout(timeout);

await server.shutdown();
