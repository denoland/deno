// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// This is not a real HTTP server. We read blindly one time into 'requestBuf',
// then write this fixed 'responseBuf'. The point of this benchmark is to
// exercise the event loop in a simple yet semi-realistic way.
const requestBuf = new Uint8Array(64 * 1024);
const responseBuf = new Uint8Array(
  "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n"
    .split("")
    .map((c) => c.charCodeAt(0)),
);

/** Listens on 0.0.0.0:4500, returns rid. */
function listen() {
  const { rid } = Deno.core.jsonOpSync("listen", {});
  return rid;
}

/** Accepts a connection, returns rid. */
async function accept(serverRid) {
  const { rid } = await Deno.core.jsonOpAsync("accept", { rid: serverRid });
  return rid;
}

/**
 * Reads a packet from the rid, presumably an http request. data is ignored.
 * Returns bytes read.
 */
async function read(rid, data) {
  const { nread } = await Deno.core.jsonOpAsync("read", { rid }, data);
  return nread;
}

/** Writes a fixed HTTP response to the socket rid. Returns bytes written. */
async function write(rid, data) {
  const { nwritten } = await Deno.core.jsonOpAsync("write", { rid }, data);
  return nwritten;
}

function close(rid) {
  Deno.core.jsonOpSync("close", { rid });
}

async function serve(rid) {
  while (true) {
    const nread = await read(rid, requestBuf);
    if (nread <= 0) {
      break;
    }

    const nwritten = await write(rid, responseBuf);
    if (nwritten < 0) {
      break;
    }
  }
  close(rid);
}

async function main() {
  Deno.core.ops();

  const listenerRid = listen();
  Deno.core.print(`http_bench_json_ops listening on http://127.0.0.1:4544/\n`);

  for (;;) {
    const rid = await accept(listenerRid);
    if (rid < 0) {
      Deno.core.print(`accept error ${rid}`);
      return;
    }
    serve(rid);
  }
}

main();
