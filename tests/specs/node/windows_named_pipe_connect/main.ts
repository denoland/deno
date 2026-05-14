// Test HTTP client over Windows named pipe using http.request({socketPath}).
// Uses net.createServer as a raw TCP server that speaks HTTP, then
// http.request({socketPath}) as the client. This exercises the
// NetworkStream::WindowsPipe extraction in take_network_stream_resource,
// which is the code path fixed by this PR.

import * as net from "node:net";
import * as http from "node:http";
import * as crypto from "node:crypto";

const pipeName = `\\\\.\\pipe\\deno_test_${crypto.randomUUID()}`;

// Raw TCP server on a named pipe that responds with a minimal HTTP response.
const server = net.createServer((socket) => {
  socket.on("data", () => {
    socket.write(
      "HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\nhello from pipe",
    );
    socket.end();
  });
});

server.listen(pipeName, () => {
  const req = http.request({ socketPath: pipeName, path: "/" }, (res) => {
    let body = "";
    res.on("data", (chunk: Buffer) => {
      body += chunk.toString();
    });
    res.on("end", () => {
      console.log(body);
      server.close();
    });
  });
  req.end();
});
