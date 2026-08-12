import { Socket } from "node:net";

// fd 3 is an inherited TCP connection registered at startup as an extra
// stdio fd. Adopting it via net.Socket({ fd }) routes through TCPWrap::open,
// which must consume the inherited registration instead of rejecting the fd
// with EEXIST.
const socket = new Socket({ fd: 3, readable: false, writable: true });

socket.on("error", (err) => {
  console.error(err);
  process.exit(1);
});

socket.end("hello from fd 3 tcp", () => {
  process.exit(0);
});
