// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
const http = require("http");
const port = process.argv[2] || "4544";
console.log("port", port);
const body = "Hello World";
http
  .Server((req, res) => {
    res.end(body);
  })
  .listen(port);
