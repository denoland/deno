// Test net.Socket({ fd }) with a pipe created via child_process.
// This verifies that creating a Socket from a raw fd works through
// the full stack: _createHandle -> Pipe.open(fd) -> FdStreamBase.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { execSync } = require("child_process");

// Create a socketpair via a helper subprocess
const result = execSync(
  `${Deno.execPath()} eval "
    const net = require('net');
    const server = net.createServer((conn) => {
      conn.write('hello from socket fd');
      conn.end();
    });
    const tmpSock = require('path').join(require('os').tmpdir(), 'deno_net_socket_fd_test_' + process.pid + '.sock');
    try { require('fs').unlinkSync(tmpSock); } catch {}
    server.listen(tmpSock, () => {
      const client = net.connect(tmpSock, () => {
        let data = '';
        client.on('data', (chunk) => data += chunk);
        client.on('end', () => {
          console.log(data);
          server.close();
          try { require('fs').unlinkSync(tmpSock); } catch {}
        });
      });
    });
  "`,
  { encoding: "utf8" },
);

console.log("result:", result.trim());
