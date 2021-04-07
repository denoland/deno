// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
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
  return Deno.core.jsonOpSync("listen");
}

/** Accepts a connection, returns rid. */
function accept(serverRid) {
  return Deno.core.jsonOpAsync("accept", serverRid);
}

/**
 * Reads a packet from the rid, presumably an http request. data is ignored.
 * Returns bytes read.
 */
function read(rid, data) {
  return Deno.core.jsonOpAsync("read", rid, data);
}

/** Writes a fixed HTTP response to the socket rid. Returns bytes written. */
function write(rid, data) {
  return Deno.core.jsonOpAsync("write", rid, data);
}

function close(rid) {
  Deno.core.jsonOpSync("close", rid);
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
  Deno.core.registerErrorClass("Error", Error);

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
