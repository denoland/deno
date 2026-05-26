// Regression test for https://github.com/denoland/deno/issues/34370
//
// When an http.Agent picks a stale socket out of freeSockets, addRequest
// sets req.reusedSocket = true before the write fails. maybeRetryRequest
// then opens a fresh TCP connection and re-sends the request. The retry
// must reset req.reusedSocket to false; otherwise userland code that
// branches on the flag sees a misleading "true" for a brand-new socket.
import http from "node:http";
import { strictEqual } from "node:assert";

const agent = new http.Agent({ keepAlive: true });

const server = http.createServer((_req, res) => {
  res.writeHead(200);
  res.end("hi");
});

let connectionCount = 0;
server.on("connection", () => connectionCount++);

await new Promise<void>((r) => server.listen(0, "127.0.0.1", () => r()));
const port = (server.address() as { port: number }).port;

function doRequest(): Promise<boolean> {
  return new Promise((resolve, reject) => {
    const req = http.request({ host: "127.0.0.1", port, path: "/", agent });
    let reused = false;
    req.on("socket", () => {
      reused = req.reusedSocket;
    });
    req.on("error", reject);
    req.on("response", (res) => {
      res.on("data", () => {});
      res.on("end", () => resolve(reused));
    });
    req.end();
  });
}

const r1 = await doRequest();
strictEqual(r1, false, "R1 should not reuse socket");
console.log("R1 reused:", r1);

// Let the socket land in freeSockets, then FIN the idle connection from
// the server side. Fire R2 synchronously before Deno processes the FIN,
// so addRequest still sees the stale socket in the free pool.
await Promise.resolve();
server.closeAllConnections();

const r2 = await doRequest();
strictEqual(r2, false, "R2 must report reusedSocket=false after retry");
console.log("R2 reused:", r2);
strictEqual(connectionCount, 2, "server should see 2 TCP connections");
console.log("connections:", connectionCount);

agent.destroy();
server.close();
