"use strict";

const assert = require("assert");
const http = require("http");

const test_req_body = "some stuff...\n";
const test_res_body = "other stuff!\n";
let sent_continue = false;
let got_continue = false;

const server = http.createServer();
server.on("checkContinue", (req, res) => {
  res.writeContinue();
  sent_continue = true;
  req.on("data", () => {});
  req.on("end", () => {
    res.writeHead(200, {
      "Content-Type": "text/plain",
      "ABCD": "1",
    });
    res.end(test_res_body);
  });
});
server.listen(0);

server.on("listening", () => {
  const req = http.request({
    port: server.address().port,
    method: "POST",
    path: "/world",
    headers: {
      "Expect": "100-continue",
      "Content-Length": test_req_body.length,
    },
  });
  let body = "";
  req.on("continue", () => {
    assert.ok(sent_continue);
    got_continue = true;
    req.end(test_req_body);
  });
  req.on("response", (res) => {
    assert.ok(got_continue, "Full response received before 100 Continue");
    assert.strictEqual(
      res.statusCode,
      200,
      `Final status code was ${res.statusCode}, not 200.`,
    );
    res.setEncoding("utf8");
    res.on("data", function (chunk) {
      body += chunk;
    });
    res.on("end", () => {
      assert.strictEqual(body, test_res_body);
      assert.ok("abcd" in res.headers, "Response headers missing.");
      console.log("ok");
      server.close();
    });
  });
});
