import { createServer } from "node:net";

// fd 3 is an inherited *listening* TCP socket. Adopting it via
// net.createServer().listen({ fd }) routes through TCPWrap::open with
// SocketType::Server (uv_tcp_open_listener), which must consume the
// inherited registration instead of rejecting the fd with EEXIST.
const server = createServer((conn) => {
  conn.end("hello from fd 3 listener", () => {
    server.close(() => process.exit(0));
  });
});

server.on("error", (err) => {
  console.error(err);
  process.exit(1);
});

server.listen({ fd: 3 });
