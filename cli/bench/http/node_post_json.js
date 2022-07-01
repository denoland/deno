// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const http = require("http");
const port = process.argv[2] || "4544";
console.log("port", port);
http
  .Server((req, res) => {
    if (req.method == "POST") {
      let body = "";
      req.on("data", function (data) {
        body += data;
      });
      req.on("end", function () {
        const { hello } = JSON.parse(body);
        res.end(hello);
      });
    }
  })
  .listen(port);
