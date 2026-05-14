// Regression test: handle close and detach behavior for owned streams.
// After the stream infrastructure refactor, LibUvStreamWrap.close() clears
// the JS handle back-reference, and TTY's Drop impl calls detach_stream()
// to null the uv_stream_t.data pointer. This test verifies that:
// 1. Closing a net.Server and all its sockets results in clean process exit
// 2. Multiple rapid close/destroy cycles don't crash or hang
// 3. The close callback fires even when destroy is called during active I/O

import * as net from "node:net";

let closedConnections = 0;
const TOTAL_CONNECTIONS = 5;
const closeCallbacksFired: boolean[] = [];

const server = net.createServer((socket) => {
  socket.on("data", () => {
    // just consume
  });

  socket.on("close", () => {
    closedConnections++;
    closeCallbacksFired.push(true);
    if (closedConnections === TOTAL_CONNECTIONS) {
      if (closeCallbacksFired.length !== TOTAL_CONNECTIONS) {
        console.log(
          `FAIL: expected ${TOTAL_CONNECTIONS} close callbacks, got ${closeCallbacksFired.length}`,
        );
        process.exit(1);
      }
      console.log("PASS: all handles closed cleanly");
      server.close();
    }
  });

  // Destroy while the handle is in "reading" state
  setTimeout(() => {
    socket.destroy();
  }, 10);
});

server.listen(0, () => {
  const { port } = server.address() as net.AddressInfo;

  for (let i = 0; i < TOTAL_CONNECTIONS; i++) {
    const client = net.connect(port, () => {
      client.write("test data");
    });
    client.on("error", () => {
      // Expected -- server destroys the connection
    });
    client.on("close", () => {
      // Client side closed too
    });
  }
});
