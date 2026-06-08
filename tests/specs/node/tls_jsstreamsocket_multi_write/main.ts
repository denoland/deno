// Regression test for https://github.com/denoland/deno/issues/33907
//
// When TLSSocket wraps a non-net.Socket Duplex stream (the mssql/tedious
// pattern), JSStreamSocket is used. The cleartext write callback (the `cb`
// passed to TLSSocket._write) only fires after InvokeQueued runs in Rust.
// For native UV streams that happens in `enc_write_cb` (libuv write
// completion). For JS-backed streams there is no `enc_write_cb`, so the
// callback was only fired indirectly when peer data arrived (via cycle()).
//
// If the writer awaits each cb before sending the next chunk, and the
// peer waits for all chunks before responding (e.g. TDS packetized
// parameter data), the second chunk's write callback never fires, the
// writable stream stays paused, and the connection deadlocks.

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

const CHUNK_SIZE = 4096;
const NUM_CHUNKS = 5;
const TOTAL_BYTES = CHUNK_SIZE * NUM_CHUNKS;

const server = tls.createServer({ cert, key }, (socket) => {
  let received = 0;
  socket.on("data", (chunk: Buffer) => {
    received += chunk.length;
    if (received >= TOTAL_BYTES) {
      socket.write("ok");
    }
  });
  socket.on("error", () => {});
  socket.on("end", () => socket.end());
});

server.listen(0, async () => {
  const { port } = server.address() as { port: number };

  const rawSocket = net.connect(port, "localhost", () => {});

  // Plain Duplex (NOT a net.Socket) - drives JSStreamSocket in _tls_wrap.js.
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

  const writeChunk = (data: Buffer) =>
    new Promise<void>((resolve, reject) => {
      tlsSocket.write(data, (err) => err ? reject(err) : resolve());
    });

  const timeout = setTimeout(() => {
    console.log("DEADLOCK");
    Deno.exit(1);
  }, 10000);

  await new Promise<void>((resolve) =>
    tlsSocket.once("secureConnect", resolve)
  );

  const ack = new Promise<void>((resolve) => {
    tlsSocket.on("data", (d: Buffer) => {
      if (d.toString() === "ok") resolve();
    });
  });

  // Await each chunk's cb before sending the next. The deadlock manifests
  // when the second cb never fires.
  for (let i = 0; i < NUM_CHUNKS; i++) {
    await writeChunk(Buffer.alloc(CHUNK_SIZE, "x"));
  }

  await ack;
  clearTimeout(timeout);
  console.log("done");
  tlsSocket.destroy();
  rawSocket.destroy();
  server.close();
});
