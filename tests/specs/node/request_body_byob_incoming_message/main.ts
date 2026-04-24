// Regression test for https://github.com/denoland/deno/issues/33392
// A `node:http` IncomingMessage used as the body of a `Request` must produce
// a byte `ReadableStream`, so `getReader({ mode: "byob" })` works.

import * as http from "node:http";

const server = http.createServer(async (req, res) => {
  const request = new Request("http://localhost/", {
    method: req.method,
    headers: req.headers as HeadersInit,
    body: req as unknown as BodyInit,
    // deno-lint-ignore no-explicit-any
    duplex: "half",
  } as any);

  const reader = request.body!.getReader({ mode: "byob" });
  const buf = new Uint8Array(32);
  const { value, done } = await reader.read(buf);
  console.log("done:", done, "len:", value?.byteLength);
  console.log("text:", new TextDecoder().decode(value));

  res.end("OK");
  server.close();
});

server.listen(0, async () => {
  const port = (server.address() as { port: number }).port;
  const res = await fetch(`http://localhost:${port}/`, {
    method: "POST",
    body: "hello world",
  });
  console.log("client got:", await res.text());
});
