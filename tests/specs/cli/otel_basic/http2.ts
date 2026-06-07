import http2 from "node:http2";

const server = http2.createServer((req, res) => {
  let status = 404;
  if (req.url === "/found") {
    status = 200;
  } else if (req.url === "/error") {
    status = 500;
  } else if (req.url === "/reset") {
    // Reset the stream so both the server and client spans exercise the
    // error fan-in in Http2Stream._destroy (updateSpanFromError).
    res.stream.close(http2.constants.NGHTTP2_INTERNAL_ERROR);
    return;
  }
  res.writeHead(status);
  res.end();
});

await new Promise<void>((resolve) => server.listen(0, () => resolve()));
const port = (server.address() as { port: number }).port;
const url = `http://localhost:${port}`;

const client = http2.connect(url);

async function request(path: string) {
  const req = client.request({ ":path": path });
  return new Promise<void>((resolve) => {
    req.on("response", () => {});
    req.on("data", () => {});
    req.on("end", () => resolve());
    req.end();
  });
}

async function requestExpectError(path: string) {
  const req = client.request({ ":path": path });
  return new Promise<void>((resolve) => {
    req.on("error", () => {});
    req.on("close", () => resolve());
    req.end();
  });
}

await request("/found");
await request("/not-found");
await request("/error");
await requestExpectError("/reset");

client.close();
server.close();
