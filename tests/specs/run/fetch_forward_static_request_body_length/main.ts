import { setTimeout } from "node:timers/promises";

const echoServer = Deno.serve(
  { hostname: "127.0.0.1", port: 0, onListen() {} },
  async (req: Request) => {
    const path = new URL(req.url).pathname;
    if (path === "/split") {
      resolveSplitStarted();
    }
    const forwarded = {
      contentLength: req.headers.get("content-length"),
      body: await req.text(),
    };
    if (path === "/split") {
      resolveSplitForwarded(forwarded);
    }
    return Response.json(forwarded);
  },
);

const echoUrl = `http://127.0.0.1:${echoServer.addr.port}/`;
const { promise: splitStartedPromise, resolve: resolveSplitStarted } = Promise
  .withResolvers<void>();
const { promise: splitForwardedPromise, resolve: resolveSplitForwarded } =
  Promise.withResolvers<{ contentLength: string | null; body: string }>();
const proxyServer = Deno.serve(
  { hostname: "127.0.0.1", port: 0, onListen() {} },
  (req: Request) => {
    const path = new URL(req.url).pathname;
    return fetch(path === "/split" ? `${echoUrl}split` : echoUrl, {
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

// Scenario 3: if the request body has only partially arrived when the proxy
// reads req.body, the forwarded request should still use the full length and
// stream all bytes.
const splitBody = JSON.stringify({
  tags: ["http-12345678-1234-1234-1234-123456789abc"],
});
const splitExpectedLength = String(encoder.encode(splitBody).byteLength);
const splitAt = splitBody.length - 3;
const conn = await Deno.connect({
  hostname: "127.0.0.1",
  port: proxyServer.addr.port,
});
await conn.write(encoder.encode(
  `POST /split HTTP/1.1\r\nhost: x\r\ncontent-length: ${splitExpectedLength}\r\n\r\n` +
    splitBody.slice(0, splitAt),
));
const splitStartedTimeout = new AbortController();
try {
  await Promise.race([
    splitStartedPromise,
    setTimeout(5_000, undefined, {
      signal: splitStartedTimeout.signal,
    }).then(() => {
      throw new Error("Timed out waiting for split request to start");
    }),
  ]);
} finally {
  splitStartedTimeout.abort();
}
await conn.write(encoder.encode(splitBody.slice(splitAt)));
const splitForwarded = await splitForwardedPromise;

console.log("split body forwarded:", splitForwarded.body === splitBody);
console.log("split content-length:", splitForwarded.contentLength);
console.log("split expected length:", splitExpectedLength);
console.log(
  "split content-length preserved:",
  splitForwarded.contentLength === splitExpectedLength,
);

try {
  conn.close();
} catch {
  // The server may already have closed the connection.
}
await proxyServer.shutdown();
await echoServer.shutdown();
