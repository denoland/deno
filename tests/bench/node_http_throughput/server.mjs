// Copyright 2018-2026 the Deno authors. MIT license.

import http from "node:http";

const body = "ok";

const server = http.createServer((_req, res) => {
  res.setHeader("Content-Length", body.length);
  res.end(body);
});

server.keepAliveTimeout = 60000;
server.headersTimeout = 65000;

server.listen(0, "127.0.0.1", () => {
  const address = server.address();
  console.log(JSON.stringify({ port: address.port }));
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => {
    server.close(() => process.exit(0));
    setTimeout(() => process.exit(1), 1000).unref();
  });
}
