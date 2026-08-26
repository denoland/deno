import { createWriteStream } from "node:fs";
import { Socket } from "node:net";

// First let node:fs claim fd 3. This dups the inherited descriptor into a
// separate handle so the original fd 3 stays free for libuv to reclaim below.
const stream = createWriteStream(null, { fd: 3 });

stream.on("error", (err) => {
  console.error(err);
  process.exit(1);
});

stream.on("close", () => {
  // After node:fs is done with its dup, libuv must still be able to claim the
  // original numeric fd 3 via net.Socket({ fd }), the path fork/cluster use.
  const socket = new Socket({ fd: 3, readable: false, writable: true });
  socket.on("error", (err) => {
    console.error(err);
    process.exit(1);
  });
  socket.end(" and from fd 3 socket", () => {
    process.exit(0);
  });
});

stream.write("hello from fd 3");
stream.end();
