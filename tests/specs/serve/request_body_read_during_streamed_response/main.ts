// Regression test for https://github.com/denoland/deno/issues/36966
//
// The handler reads part of the request body, keeps reading the rest in the
// background and answers with a streamed response. The client only sends the
// rest of the body after it received the whole response. The background read
// must still be woken for those bytes and read the body to the end.

const TOTAL = 8 * 1024 * 1024;
const FIRST = 1024 * 1024;
const RESPONSE_SIZE = 1024 * 1024;

const { promise: bodyRead, resolve: resolveBodyRead } = Promise
  .withResolvers<number>();

const server = Deno.serve(
  { port: 0, hostname: "127.0.0.1", onListen: () => {} },
  async (req) => {
    const reader = req.body!.getReader();
    let read = 0;
    while (read <= 256 * 1024) {
      const { value, done } = await reader.read();
      if (done) break;
      read += value.byteLength;
    }
    (async () => {
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        read += value.byteLength;
      }
      resolveBodyRead(read);
    })();
    let left = RESPONSE_SIZE;
    return new Response(
      new ReadableStream({
        pull(controller) {
          const n = Math.min(65536, left);
          left -= n;
          controller.enqueue(new Uint8Array(n));
          if (left === 0) controller.close();
        },
      }),
      { headers: { "content-length": String(RESPONSE_SIZE) } },
    );
  },
);

const timeout = setTimeout(() => {
  console.log("timed out waiting for the request body to be read");
  Deno.exit(1);
}, 10_000);

const conn = await Deno.connect({
  port: server.addr.port,
  hostname: "127.0.0.1",
});
await conn.write(
  new TextEncoder().encode(
    `PUT / HTTP/1.1\r\nHost: x\r\nContent-Length: ${TOTAL}\r\n\r\n`,
  ),
);
// Write the first part of the body concurrently with reading the response.
const firstWrite = (async () => {
  const chunk = new Uint8Array(FIRST);
  let written = 0;
  while (written < chunk.length) {
    written += await conn.write(chunk.subarray(written));
  }
})();

// Read until the whole response (head and RESPONSE_SIZE body bytes) arrived.
const buf = new Uint8Array(65536);
let head: string | null = "";
let received = 0;
while (true) {
  const n = await conn.read(buf);
  if (n === null) throw new Error("connection closed early");
  if (head === null) {
    received += n;
  } else {
    head += new TextDecoder("latin1").decode(buf.subarray(0, n));
    const end = head.indexOf("\r\n\r\n");
    if (end !== -1) {
      received += head.length - (end + 4);
      head = null;
    }
  }
  if (received >= RESPONSE_SIZE) break;
}
await firstWrite;
console.log("response received");

const rest = new Uint8Array(TOTAL - FIRST);
let written = 0;
while (written < rest.length) {
  written += await conn.write(rest.subarray(written));
}

console.log("body read:", await bodyRead, "of", TOTAL);
clearTimeout(timeout);

conn.close();
await server.shutdown();
