// When a request path is rewritten to absolute-form for proxying, `url.full`
// must report it as-is rather than appending it to the authority again.
// Refs: https://github.com/nodejs/node/issues/59625
import http from "node:http";
import net from "node:net";
import { text } from "node:stream/consumers";

// A raw TCP stand-in for the proxy: it short-circuits every request with a
// canned response. Using node:net rather than node:http keeps the trace to the
// single client span under test, with no server span or http.server metrics.
const proxy = net.createServer((socket) => {
  socket.once("data", () => {
    socket.end("HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nvia-proxy");
  });
});
await new Promise<void>((resolve) =>
  proxy.listen(0, "127.0.0.1", () => resolve())
);
const proxyPort = (proxy.address() as net.AddressInfo).port;

await new Promise<void>((resolve) => {
  http.request({
    hostname: "127.0.0.1",
    port: 8080,
    path: "/foo",
    agent: new http.Agent({
      proxyEnv: { HTTP_PROXY: `http://127.0.0.1:${proxyPort}` },
      // deno-lint-ignore no-explicit-any
    } as any),
  }, async (res) => {
    await text(res);
    resolve();
  }).end();
});

proxy.close();
