// Regression test for https://github.com/denoland/deno/issues/28261.
//
// A node:http server using the direct parser-consume path must be able to
// start a nested node:http client request from its first request callback.
// Older builds answered that first inbound request with a generated 400 under
// `deno test`, while the outgoing proxy request never reached the target.

import { strictEqual } from "node:assert";
import http from "node:http";
import net from "node:net";

Deno.test("first request to a node:http proxy under deno test", async () => {
  let targetRequests = 0;
  const target = http.createServer((_req, res) => {
    targetRequests++;
    res.writeHead(200);
    res.end("ok");
  });

  await listen(target);
  const targetPort = (target.address() as net.AddressInfo).port;

  const proxy = http.createServer((req, res) => {
    req.resume();
    const proxyReq = http.request({
      host: "127.0.0.1",
      port: targetPort,
      method: "GET",
      path: "/",
    }, (proxyRes) => {
      res.writeHead(proxyRes.statusCode ?? 502);
      proxyRes.pipe(res);
    });
    proxyReq.on("error", () => {
      res.writeHead(502);
      res.end();
    });
    proxyReq.end();
  });

  await listen(proxy);
  const proxyPort = (proxy.address() as net.AddressInfo).port;

  try {
    const r1 = await request(proxyPort);
    const r2 = await request(proxyPort);

    strictEqual(r1.status, 200);
    strictEqual(r2.status, 200);
    strictEqual(r1.body, "ok");
    strictEqual(r2.body, "ok");
    strictEqual(targetRequests, 2);

    console.log("R1", r1.status);
    console.log("R2", r2.status);
    console.log("target requests:", targetRequests);
  } finally {
    await close(proxy);
    await close(target);
  }
});

function listen(server: http.Server): Promise<void> {
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", resolve);
  });
}

function close(server: http.Server): Promise<void> {
  return new Promise((resolve, reject) => {
    server.close((err) => err ? reject(err) : resolve());
  });
}

function request(port: number): Promise<{ status: number; body: string }> {
  return new Promise((resolve, reject) => {
    const req = http.get({
      host: "127.0.0.1",
      port,
      path: "/",
    }, (res) => {
      res.setEncoding("utf8");
      let body = "";
      res.on("data", (chunk) => body += chunk);
      res.on("end", () => {
        resolve({ status: res.statusCode ?? 0, body });
      });
    });
    req.on("error", reject);
  });
}
