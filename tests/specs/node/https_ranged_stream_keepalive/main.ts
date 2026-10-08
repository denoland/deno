// Regression test for https://github.com/denoland/deno/issues/36965
// Piping a one-byte ranged fs.ReadStream into an https.Server response
// answers that request, then never answers the next one on the connection.
// res.end() writes an empty chunk while the body write is still buffered,
// and the TLS zero-byte write used to drop its completion callback.

import https from "node:https";
import tls from "node:tls";
import { createReadStream, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

const file = join(tmpdir(), `deno-https-range-${Deno.pid}.txt`);
writeFileSync(file, "x");

const server = https.createServer({ key, cert }, (req, res) => {
  if (req.url === "/") {
    res.end("ok");
    return;
  }
  res.setHeader("Content-Length", "1");
  createReadStream(file, { start: 0, end: 0 }).pipe(res);
});

function readOne(
  buf: string,
): { status: string; body: string; rest: string } | null {
  const headerEnd = buf.indexOf("\r\n\r\n");
  if (headerEnd < 0) return null;
  const header = buf.slice(0, headerEnd);
  const status = header.split("\r\n")[0] ?? "";
  const match = header.match(/content-length:\s*(\d+)/i);
  if (!match) return null;
  const len = Number(match[1]);
  const bodyStart = headerEnd + 4;
  if (buf.length < bodyStart + len) return null;
  return {
    status,
    body: buf.slice(bodyStart, bodyStart + len),
    rest: buf.slice(bodyStart + len),
  };
}

server.listen(0, "127.0.0.1", () => {
  const { port } = server.address() as { port: number };
  const socket = tls.connect({
    port,
    host: "127.0.0.1",
    rejectUnauthorized: false,
  });

  let buf = "";
  let sentNext = false;
  const timer = setTimeout(() => {
    console.log("timeout");
    socket.destroy();
    server.close();
  }, 4000);

  socket.on("secureConnect", () => {
    socket.write(
      "GET /file HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n",
    );
  });

  socket.on("data", (chunk: Uint8Array) => {
    buf += new TextDecoder().decode(chunk);
    const first = readOne(buf);
    if (!first) return;
    if (!sentNext) {
      sentNext = true;
      console.log(`file: ${first.status} body=${first.body}`);
      buf = first.rest;
      socket.write(
        "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n",
      );
    }
    const second = readOne(buf);
    if (!second) return;
    console.log(`next: ${second.status} body=${second.body}`);
    console.log("ok");
    clearTimeout(timer);
    socket.end();
    server.close();
  });

  socket.on("error", (err: Error) => {
    console.log(`error: ${err.message}`);
    clearTimeout(timer);
    server.close();
  });
});
