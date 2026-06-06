import http2 from "node:http2";

const server = http2.createServer((req, res) => {
  let status = 404;
  if (req.url === "/found") {
    status = 200;
  } else if (req.url === "/error") {
    status = 500;
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

await request("/found");
await request("/not-found");
await request("/error");

client.close();
server.close();
