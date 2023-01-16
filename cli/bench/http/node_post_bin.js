// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
const http = require("http");
const port = process.argv[2] || "4544";
console.log("port", port);
http
  .Server((req, res) => {
    if (req.method == "POST") {
      let chunks = [];
      req.on("data", function (data) {
        chunks.push(data);
      });
      req.on("end", function () {
        const buffer = Buffer.concat(chunks);
        res.end(buffer.byteLength.toString());
      });
    }
  })
  .listen(port);
