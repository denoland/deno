// Regression test for https://github.com/denoland/deno/issues/33713
// Writing a Uint8Array with a detached ArrayBuffer through TLSWrap
// should not panic.

import * as tls from "node:tls";
import * as net from "node:net";
import { readFileSync } from "node:fs";

const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
  "utf8",
);
const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
  "utf8",
);

const server = tls.createServer({ key, cert }, (socket) => {
  socket.on("data", () => {});
  socket.on("error", () => {});
});

server.listen(0, () => {
  const port = (server.address() as net.AddressInfo).port;
  const client = tls.connect(
    { port, host: "localhost", rejectUnauthorized: false },
    () => {
      // deno-lint-ignore no-explicit-any
      const handle = (client as any)._handle;

      const ab = new ArrayBuffer(16);
      const view = new Uint8Array(ab);
      view.set([1, 2, 3, 4]);
      // Detach the backing ArrayBuffer
      structuredClone(ab, { transfer: [ab] });

      // Call native handle methods directly with the detached buffer.
      // Before the fix these would panic with "unwrap() on None".
      handle.writeBuffer({}, view);
      handle.writev({}, [view], true);
      handle.writev({}, [view, "buffer"], false);

      // Mixed valid + detached in all_buffers=true: the detached chunk is
      // silently skipped while the valid chunk is still written.
      const validBuf = new Uint8Array([5, 6, 7, 8]);
      handle.writev({}, [view, validBuf], true);

      // Mixed detached buffer + valid string in all_buffers=false: verifies
      // that skipping the detached chunk does not desync the paired indexing.
      handle.writev({}, [view, "utf8", "hello", "utf8"], false);

      console.log("ok");

      client.destroy();
      server.close();
    },
  );
  client.on("error", () => {});
});
