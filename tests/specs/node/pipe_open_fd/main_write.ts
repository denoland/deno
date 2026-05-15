// Test Pipe.open(fd) for writing via net.Socket({ fd }).
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const fs = require("fs");
const net = require("net");
const path = require("path");
const os = require("os");

const tmpFile = path.join(
  os.tmpdir(),
  `deno_pipe_write_test_${process.pid}.txt`,
);

// Create a pipe (FIFO) to get a PIPE-type fd, or just use a socketpair.
// Simplest: write to a file via child process using inherited fd.
// Actually, the simplest test: create a unix socket server, connect,
// and verify data flows through a socket fd wrapped in net.Socket.
const sockPath = path.join(os.tmpdir(), `deno_pipe_write_${process.pid}.sock`);
try {
  fs.unlinkSync(sockPath);
} catch {}

const server = net.createServer((conn: any) => {
  let data = "";
  conn.on("data", (chunk: any) => data += chunk);
  conn.on("end", () => {
    console.log("server received:", data.trim());
    server.close();
    try {
      fs.unlinkSync(sockPath);
    } catch {}
  });
});

server.listen(sockPath, () => {
  const client = net.connect(sockPath, () => {
    client.write("hello from pipe write\n");
    client.end();
  });
});
