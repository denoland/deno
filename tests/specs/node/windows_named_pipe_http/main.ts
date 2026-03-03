// Test HTTP over Windows named pipes using Node.js compat layer.
// Creates a named pipe server and makes an HTTP request to it,
// verifying the full round-trip works (similar to Docker API access).

import * as http from "node:http";
import * as net from "node:net";
import * as crypto from "node:crypto";

const pipeName = `\\\\.\\pipe\\deno_test_${crypto.randomUUID()}`;

// Create a simple HTTP server on a named pipe
const server = http.createServer((_req, res) => {
  res.writeHead(200, { "Content-Type": "application/json" });
  res.end(JSON.stringify({ status: "ok", pipe: pipeName }));
});

server.listen(pipeName, () => {
  // Make an HTTP request over the named pipe
  const req = http.request(
    {
      socketPath: pipeName,
      path: "/test",
      method: "GET",
    },
    (res) => {
      let data = "";
      res.on("data", (chunk: Buffer) => {
        data += chunk.toString();
      });
      res.on("end", () => {
        const parsed = JSON.parse(data);
        console.log(`status: ${parsed.status}`);
        console.log(
          `pipe: ${parsed.pipe === pipeName ? "matches" : "mismatch"}`,
        );
        console.log(`statusCode: ${res.statusCode}`);
        server.close();
      });
    },
  );

  req.on("error", (err: Error) => {
    console.error(`request error: ${err.message}`);
    server.close();
  });

  req.end();
});
