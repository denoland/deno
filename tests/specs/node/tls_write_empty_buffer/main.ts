// Regression test for https://github.com/denoland/deno/issues/34404
// Writing an empty (zero-length) Uint8Array through TLSWrap should not
// panic. A zero-length ArrayBuffer has a null backing-store data pointer,
// so `ArrayBuffer::data()` returns `None`; the write ops must skip such
// chunks instead of unwrapping. This is the path hit by @deno/sandbox.

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

      // A zero-length view: its ArrayBuffer's data pointer is null, so
      // `ab.data()` returns `None`. Before the fix these calls panicked
      // with "called `Option::unwrap()` on a `None` value".
      const empty = new Uint8Array(0);
      handle.writeBuffer({}, empty);
      handle.writev({}, [empty], true);
      handle.writev({}, [empty, "buffer"], false);

      // Mixed empty + valid in all_buffers=true: the empty chunk is
      // skipped while the valid chunk is still written.
      const validBuf = new Uint8Array([5, 6, 7, 8]);
      handle.writev({}, [empty, validBuf], true);

      // Mixed empty buffer + valid string in all_buffers=false: skipping
      // the empty chunk must not desync the paired (chunk, encoding) indexing.
      handle.writev({}, [empty, "utf8", "hello", "utf8"], false);

      // The natural socket.write() path with empty buffers (what a library
      // such as @deno/sandbox triggers). cork()/uncork() batches the writes
      // into a single writev with an empty chunk interleaved.
      client.cork();
      client.write(new Uint8Array(0));
      client.write(Buffer.from("hello"));
      client.write(Buffer.alloc(0));
      client.uncork();
      client.write(new Uint8Array(0));

      console.log("ok");

      client.destroy();
      server.close();
    },
  );
  client.on("error", () => {});
});
