// A node:http request routed through a proxy connects the socket only to the
// proxy, so the request target must be permission-checked separately (matching
// fetch()). NODE_USE_ENV_PROXY=1 and HTTP_PROXY are set by the test runner.
//
// The proxy here answers directly rather than forwarding (forwarding would loop
// since the proxy's own request would be re-routed through the global proxy).
// The "blocked" case never reaches the proxy: the target check throws first.

import http from "node:http";

const PROXY_PORT = 9457;
const TARGET = "http://127.0.0.1:9458/test";

const proxy = http.createServer((_req, res) => res.end("hello from origin"));
await new Promise((resolve) => proxy.listen(PROXY_PORT, "127.0.0.1", resolve));

try {
  const body = await new Promise((resolve, reject) => {
    const req = http.get(TARGET, (res) => {
      let data = "";
      res.setEncoding("utf8");
      res.on("data", (c) => data += c);
      res.on("end", () => resolve(data));
    });
    req.on("error", reject);
  });
  console.log("result: ok", body);
} catch (e) {
  console.log("result:", e.name);
}

proxy.close();
