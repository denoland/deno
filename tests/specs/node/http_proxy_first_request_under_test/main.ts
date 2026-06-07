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
    const r1 = statusLine(await rawRequest(proxyPort));
    const r2 = statusLine(await rawRequest(proxyPort));

    strictEqual(r1, "HTTP/1.1 200 OK");
    strictEqual(r2, "HTTP/1.1 200 OK");
    strictEqual(targetRequests, 2);

    console.log("R1", r1);
    console.log("R2", r2);
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

function rawRequest(port: number): Promise<string> {
  return new Promise((resolve, reject) => {
    const socket = net.connect(port, "127.0.0.1", () => {
      socket.write("GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
    });
    let response = "";
    socket.setTimeout(5000, () => {
      socket.destroy(new Error("timed out waiting for proxy response"));
    });
    socket.on("data", (chunk) => response += chunk);
    socket.on("error", reject);
    socket.on("close", () => resolve(response));
  });
}

function statusLine(response: string): string {
  return response.split("\r\n", 1)[0];
}
