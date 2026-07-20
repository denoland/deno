// Coverage for an `https.request` tunneled over `tls.connect({ socket })`
// where the underlying transport is a plain Duplex (the JSStreamSocket path
// used by e.g. mssql/tedious). The server responds and immediately FINs
// (`connection: close`).
//
// The cleartext write completion for a JS-backed stream fires from `cycle()`
// (there is no libuv `enc_write_cb`). That path must run the completion
// synchronously: deferring it a macrotask can land the request's writable
// `'finish'` after the peer's `'end'`/`'close'` and abort the in-flight
// request with ERR_STREAM_PREMATURE_CLOSE. Only the write op itself (which
// holds the OpState borrow) defers; see cli change for #35820. This test
// exercises the full request/response+FIN round trip over that path.

import tls from "node:tls";
import net from "node:net";
import https from "node:https";
import { readFileSync } from "node:fs";
import { Duplex } from "node:stream";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

const server = https.createServer({ cert, key }, (_req, res) => {
  res.end("hello");
});

server.listen(0, () => {
  const { port } = server.address() as { port: number };

  const raw = net.connect(port, "localhost");
  raw.on("error", () => {});

  // Plain Duplex (NOT a net.Socket) -> drives JSStreamSocket in _tls_wrap.js.
  const wrapper = new Duplex({
    read() {},
    write(chunk: Buffer, _enc: string, cb: () => void) {
      if (raw.destroyed) return cb();
      raw.write(chunk, cb);
    },
  });
  raw.on("data", (d: Buffer) => wrapper.push(d));
  raw.on("end", () => wrapper.push(null));

  const timeout = setTimeout(() => {
    console.log("TIMEOUT");
    Deno.exit(1);
  }, 10000);

  const req = https.request({
    createConnection: () =>
      tls.connect({
        socket: wrapper,
        rejectUnauthorized: false,
        servername: "localhost",
      }) as unknown as net.Socket,
    method: "GET",
    path: "/",
    headers: { connection: "close" },
  }, (res) => {
    let body = "";
    res.on("data", (c: Buffer) => body += c);
    res.on("end", () => {
      clearTimeout(timeout);
      console.log(body === "hello" ? "done" : "BAD_BODY:" + body);
      raw.destroy();
      server.close();
    });
  });
  req.on("error", (e: Error & { code?: string }) => {
    clearTimeout(timeout);
    console.log("REQ_ERROR:" + (e.code ?? e.message));
    raw.destroy();
    server.close();
  });
  req.end();
});
