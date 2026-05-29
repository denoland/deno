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

const body = JSON.stringify({ tags: ["http-test-tag"] });
const expectedLength = String(new TextEncoder().encode(body).byteLength);
const resp = await fetch(`http://127.0.0.1:${proxyServer.addr.port}/`, {
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

await proxyServer.shutdown();
await echoServer.shutdown();

export {};
