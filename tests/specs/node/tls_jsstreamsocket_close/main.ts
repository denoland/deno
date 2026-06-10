// Regression test for https://github.com/denoland/deno/issues/20594
//
// When TLSSocket wraps a non-net.Socket stream (e.g. mssql/tedious TDS
// framing), JSStreamSocket is created. After the TLS handshake completes,
// handle[kStreamBaseField] points to the JSStreamSocket. When the socket is
// destroyed, _onClose() calls kStreamBaseField?.close() — but JSStreamSocket
// had no close() method, so the pending core.read() was never cancelled,
// causing the process to hang.
//
// This test completes a real TLS handshake over a custom Duplex stream so that
// kStreamBaseField is set, then destroys the socket and verifies the process
// exits.

import tls from "node:tls";
import net from "node:net";
import { readFileSync } from "node:fs";
import { Duplex } from "node:stream";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

const server = tls.createServer({ cert, key }, (socket) => {
  socket.on("error", () => {});
  socket.on("end", () => socket.end());
});

server.listen(0, () => {
  const { port } = server.address() as { port: number };

  // Raw TCP connection to the TLS server.
  const rawSocket = net.connect(port, "localhost", () => {
    // Wrap rawSocket in a plain Duplex — NOT a net.Socket.
    // This triggers JSStreamSocket in _tls_wrap.js, just like
    // mssql/tedious does for TLS-over-TDS.
    const wrapper = new Duplex({
      read() {},
      write(chunk: Buffer, _enc: string, cb: () => void) {
        if (rawSocket.destroyed) {
          cb();
          return;
        }
        rawSocket.write(chunk, cb);
      },
    });
    rawSocket.on("data", (d: Buffer) => wrapper.push(d));
    rawSocket.on("end", () => wrapper.push(null));

    const tlsSocket = tls.connect({
      socket: wrapper,
      rejectUnauthorized: false,
    });
    tlsSocket.on("error", () => {});

    tlsSocket.on("secureConnect", () => {
      // Handshake completed — handle[kStreamBaseField] = JSStreamSocket.

      // Remove the "close" event listeners so the ONLY cleanup path is
      // _onClose() -> kStreamBaseField?.close() -> JSStreamSocket.close().
      wrapper.removeAllListeners("close");
      tlsSocket.removeAllListeners("close");

      // Destroy everything. Without JSStreamSocket.close(), the pending
      // core.read() inside the JSStreamSocket init loop would never
      // resolve, keeping the event loop alive indefinitely.
      tlsSocket.destroy();
      rawSocket.destroy();
      server.close();

      console.log("done");
    });
  });
});
