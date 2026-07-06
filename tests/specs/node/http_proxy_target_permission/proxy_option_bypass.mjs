// A caller must not be able to bypass the proxied-target permission check by
// passing the internal `_proxy*` request options directly. No env proxy is
// configured here: the request manually sets `_proxy` to a granted proxy while
// the target is not granted. The runtime must ignore the caller-supplied
// `_proxy` so the request cannot reach the ungranted target through it,
// surfacing NotCapable instead.
//
// Proxy and target ports are taken from argv so this case can use a distinct
// pair from the other tests.

import http from "node:http";

const PROXY_PORT = Number(Deno.args[0] ?? 9457);
const TARGET_PORT = Number(Deno.args[1] ?? 9458);

const proxy = http.createServer((_req, res) => res.end("hello from origin"));
await new Promise((resolve) => proxy.listen(PROXY_PORT, "127.0.0.1", resolve));

try {
  const body = await new Promise((resolve, reject) => {
    const req = http.request({
      host: "127.0.0.1",
      port: TARGET_PORT,
      path: "/test",
      // Internal transport fields a caller should never be allowed to set.
      _proxy: { hostname: "127.0.0.1", port: PROXY_PORT, protocol: "http:" },
      _proxyProtocol: "http:",
    }, (res) => {
      let data = "";
      res.setEncoding("utf8");
      res.on("data", (c) => data += c);
      res.on("end", () => resolve(data));
    });
    req.on("error", reject);
    req.end();
  });
  console.log("result: ok", body);
} catch (e) {
  console.log("result:", e.name);
}

proxy.close();
