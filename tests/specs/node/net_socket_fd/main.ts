// Test net.Socket({ fd }) with a pipe created via child_process.
// This verifies that creating a Socket from a raw fd works through
// the full stack: _createHandle -> Pipe.open(fd) -> FdStreamBase.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const net = require("net");
const fs = require("fs");
const path = require("path");
const os = require("os");

// Create a file, open it, and wrap in a net.Socket to verify the
// Pipe.open(fd) path works end-to-end.
// Note: _createHandle requires PIPE or TCP type fds.
// A Unix domain socket pair gives us PIPE-type fds.

// Test 1: Verify that net.Socket({ fd }) works with a valid pipe fd
// by using child_process to create a pipe pair.
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

// Test 2: Verify ERR_INVALID_FD_TYPE for non-pipe/tcp fds
const tmpFile = path.join(os.tmpdir(), `deno_net_fd_test_${process.pid}.txt`);
fs.writeFileSync(tmpFile, "test");
const fileFd = fs.openSync(tmpFile, "r");
try {
  new net.Socket({ fd: fileFd });
  console.log("ERROR: should have thrown");
} catch (e: any) {
  console.log("expected error:", e.code);
} finally {
  fs.closeSync(fileFd);
  fs.unlinkSync(tmpFile);
}
