// Regression test for https://github.com/denoland/deno/issues/33377
// HTTPS PUT with body > 48KB on a keepalive-reused connection would hang
// because clear_in() rate-limits to 48KB and the remainder was never drained.

import https from "node:https";
import { readFileSync } from "node:fs";
import { Buffer } from "node:buffer";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

const server = https.createServer({ key, cert }, (req, res) => {
  let bodyLen = 0;
  req.on("data", (chunk: Buffer) => {
    bodyLen += chunk.length;
  });
  req.on("end", () => {
    res.writeHead(200);
    res.end(String(bodyLen));
  });
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
    // First request establishes the keepalive connection
    const r1 = await doRequest(port, 1024);
    console.log(`req1 (1KB): status=${r1.status} received=${r1.body}`);

    // Second request reuses the connection with a body > 48KB
    const r2 = await doRequest(port, 65536);
    console.log(`req2 (64KB): status=${r2.status} received=${r2.body}`);

    // Third request: even larger body
    const r3 = await doRequest(port, 262144);
    console.log(`req3 (256KB): status=${r3.status} received=${r3.body}`);

    console.log("ok");
  } catch (e) {
    console.error("FAILED:", (e as Error).message);
  } finally {
    agent.destroy();
    server.close();
  }
});
