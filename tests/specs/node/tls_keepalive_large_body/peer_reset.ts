// Error-path regression test for the clear_in() drain fix in enc_write_cb.
// When the peer resets the connection mid-upload during chained clear_in
// rounds, the client must surface the write error promptly (not hang or
// report success).

import https from "node:https";
import { readFileSync } from "node:fs";
import { Buffer } from "node:buffer";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

let requestCount = 0;
const server = https.createServer({ key, cert }, (req, res) => {
  requestCount++;
  const current = requestCount;

  if (current <= 1) {
    // First request: respond normally to establish keepalive
    let bodyLen = 0;
    req.on("data", (chunk: Buffer) => {
      bodyLen += chunk.length;
    });
    req.on("end", () => {
      res.writeHead(200);
      res.end(String(bodyLen));
    });
  } else {
    // Second request: destroy the socket after the first chunk arrives
    req.on("data", () => {
      req.socket.destroy();
    });
  }
});

const agent = new https.Agent({ keepAlive: true });

function doRequest(
  port: number,
  size: number,
): Promise<{ status: number; body: string }> {
  return new Promise((resolve, reject) => {
    const body = Buffer.alloc(size, "x");
    const req = https.request(
      {
        hostname: "127.0.0.1",
        port,
        path: "/",
        method: "PUT",
        headers: { "Content-Length": body.length },
        rejectUnauthorized: false,
        agent,
      },
      (res) => {
        let data = "";
        res.on("data", (chunk: string) => (data += chunk));
        res.on("end", () => resolve({ status: res.statusCode!, body: data }));
      },
    );
    req.on("error", reject);
    req.setTimeout(10000, () => {
      req.destroy(new Error("timeout"));
    });
    req.end(body);
  });
}

server.listen(0, "127.0.0.1", async () => {
  const { port } = server.address() as { port: number };
  try {
    // Establish keepalive connection
    const r1 = await doRequest(port, 1024);
    console.log(`req1: status=${r1.status}`);

    // Large upload on reused connection -- server will reset mid-stream
    await doRequest(port, 262144);
    console.log("ERROR: should not succeed");
  } catch (e) {
    const err = e as NodeJS.ErrnoException;
    // Must get a connection error, not a timeout or silent success
    const ok = err.code === "ECONNRESET" ||
      err.code === "ERR_SOCKET_CLOSED" ||
      err.message === "socket hang up";
    console.log(`req2 error surfaced: ${ok} (code=${err.code})`);
  } finally {
    agent.destroy();
    server.close();
  }
});
