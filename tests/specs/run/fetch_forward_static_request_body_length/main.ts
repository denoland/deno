const echoServer = Deno.serve(
  { hostname: "127.0.0.1", port: 0, onListen() {} },
  async (req: Request) => {
    return Response.json({
      contentLength: req.headers.get("content-length"),
      body: await req.text(),
    });
  },
);

const echoUrl = `http://127.0.0.1:${echoServer.addr.port}/`;
const proxyServer = Deno.serve(
  { hostname: "127.0.0.1", port: 0, onListen() {} },
  (req: Request) => {
    return fetch(echoUrl, {
      method: "POST",
      body: req.body,
    });
  },
);

const proxyUrl = `http://127.0.0.1:${proxyServer.addr.port}/`;
const encoder = new TextEncoder();
const body = JSON.stringify({ tags: ["http-test-tag"] });

// Scenario 1: forwarding a static request body should preserve the exact
// Content-Length from the original body.
const expectedLength = String(encoder.encode(body).byteLength);
const resp = await fetch(proxyUrl, {
  method: "POST",
  body,
});
const forwarded = await resp.json();

console.log("body forwarded:", forwarded.body === body);
console.log("content-length:", forwarded.contentLength);
console.log("expected length:", expectedLength);
console.log(
  "content-length preserved:",
  forwarded.contentLength === expectedLength,
);

// Scenario 2: forwarding an arbitrary user-created ReadableStream should not
// synthesize a Content-Length. Only Deno-created static streams are known-length.
const streamResp = await fetch(proxyUrl, {
  method: "POST",
  body: new ReadableStream({
    start(controller) {
      controller.enqueue(encoder.encode(body));
      controller.close();
    },
  }),
  duplex: "half",
});
const streamForwarded = await streamResp.json();

console.log("stream body forwarded:", streamForwarded.body === body);
console.log("stream content-length:", streamForwarded.contentLength);
console.log(
  "stream content-length unknown:",
  streamForwarded.contentLength === null,
);

await proxyServer.shutdown();
await echoServer.shutdown();
