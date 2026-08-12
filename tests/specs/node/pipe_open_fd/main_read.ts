// Test Pipe.open(fd) for reading via net.Socket({ fd }).
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const fs = require("fs");
const net = require("net");
const path = require("path");
const os = require("os");

const sockPath = path.join(os.tmpdir(), `deno_pipe_read_${process.pid}.sock`);
try {
  fs.unlinkSync(sockPath);
} catch {}

const server = net.createServer((conn: any) => {
  conn.write("hello from pipe read");
  conn.end();
});

server.listen(sockPath, () => {
  const client = net.connect(sockPath, () => {
    let data = "";
    client.on("data", (chunk: any) => data += chunk);
    client.on("end", () => {
      console.log("client received:", data.trim());
      server.close();
      try {
        fs.unlinkSync(sockPath);
      } catch {}
    });
  });
});
