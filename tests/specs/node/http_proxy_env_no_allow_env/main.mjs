// Run with only --allow-net (NO --allow-env). NODE_USE_ENV_PROXY=1 and
// HTTP_PROXY are set in the environment by the test runner.
//
// Before the fix, the proxy layer read these vars through process.env, which is
// permission-checked: without --allow-env they came back undefined, no proxy
// was configured, and the request went direct (the proxy got nothing).
// After the fix they are read with op_get_env_no_permission_check, so the
// request is routed through the proxy even without --allow-env.

import http from "node:http";

const PROXY_PORT = 9447;
const TARGET = "http://127.0.0.1:9448/test";

const proxyLogs = [];

// Minimal proxy that answers requests directly. Routing through the proxy is
// what we're checking, so it just records the (absolute-form) request-target
// and replies. Forwarding to a real origin would loop, since the proxy's own
// outgoing request would be re-routed through the global proxy too.
const proxy = http.createServer((req, res) => {
  proxyLogs.push(req.url);
  res.end("hello from origin");
});

await new Promise((resolve) => proxy.listen(PROXY_PORT, "127.0.0.1", resolve));

const body = await new Promise((resolve, reject) => {
  const req = http.get(TARGET, (res) => {
    let data = "";
    res.setEncoding("utf8");
    res.on("data", (c) => data += c);
    res.on("end", () => resolve(data));
  });
  req.on("error", reject);
});

proxy.close();

console.log("body:", body);
console.log("proxy saw requests:", proxyLogs.length);
console.log("proxy first url:", proxyLogs[0]);
