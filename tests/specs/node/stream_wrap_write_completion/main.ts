// Regression test: synchronous native write completion must not double-fire
// req.oncomplete. After the stream infrastructure refactor, Rust may complete
// a write synchronously (kLastWriteWasAsync=0) while the JS polyfill path
// also schedules oncomplete. The req.async guard prevents double-fire.

import * as net from "node:net";

const server = net.createServer((socket) => {
  socket.on("data", (data: Buffer) => {
    socket.end(data);
  });
});

server.listen(0, () => {
  const { port } = server.address() as net.AddressInfo;
  const client = net.connect(port, () => {
    let writeCallbackCount = 0;

    // Small write — likely completes synchronously in the native layer
    client.write("hello", () => {
      writeCallbackCount++;
    });

    client.on("data", () => {
      // data received
    });

    client.on("end", () => {
      client.end();
    });

    client.on("close", () => {
      if (writeCallbackCount !== 1) {
        console.log(
          `FAIL: write callback fired ${writeCallbackCount} times, expected 1`,
        );
        process.exit(1);
      }
      console.log("PASS: write callback fired exactly once");
      server.close();
    });
  });
});
