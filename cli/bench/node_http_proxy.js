// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
const http = require("http");
const port = process.argv[2] || "4544";
const originPort = process.argv[3] || "4545";
console.log("port", port);
http
  .Server((req, res) => {
    const options = {
      port: originPort,
      path: req.url,
      method: req.method,
      headers: req.headers,
    };

    const proxy = http.request(options, (proxyRes) => {
      res.writeHead(proxyRes.statusCode, proxyRes.headers);
      proxyRes.pipe(res, { end: true });
    });

    req.pipe(proxy, { end: true });
  })
  .listen(port);
