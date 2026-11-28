// Regression test: a socket registered with `onread.buffer` (Node's static
// read-buffer option) keeps reading into native memory that points at the
// Uint8Array's backing store. Detaching or transferring that ArrayBuffer
// after registration must not leave the native read path writing into freed
// memory. The handle holds the backing store alive, so the connection keeps
// working and valid input is still delivered.

import * as net from "node:net";

const PAYLOAD = 65536;

const server = net.createServer((socket) => {
  socket.write(Buffer.alloc(PAYLOAD, 0x41));
});

server.listen(0, "127.0.0.1", () => {
  const { port } = server.address() as net.AddressInfo;
  const buf = new Uint8Array(PAYLOAD);
  let received = 0;

  const client = net.connect({
    port,
    host: "127.0.0.1",
    onread: {
      buffer: buf,
      callback(nread: number) {
        received += nread;
        if (received >= PAYLOAD) {
          console.log("PASS: delivered", received, "bytes after detach");
          client.destroy();
          server.close();
        }
        return true;
      },
    },
  });

  client.on("connect", () => {
    // Transfer the registered buffer away and collect the original backing
    // store. The native read path must not touch freed memory afterwards.
    buf.buffer.transfer(16);
    (globalThis as { gc?: () => void }).gc?.();
  });

  client.on("error", (err) => {
    console.log("FAIL: client error", err.message);
    server.close();
  });
});
