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
  return Deno.core.opSync("listen");
}

/** Accepts a connection, returns rid. */
function accept(serverRid) {
  return Deno.core.opAsync("accept", serverRid);
}

/**
 * Reads a packet from the rid, presumably an http request. data is ignored.
 * Returns bytes read.
 */
function read(rid, data) {
  return Deno.core.opAsync("read", rid, data);
}

/** Writes a fixed HTTP response to the socket rid. Returns bytes written. */
function write(rid, data) {
  return Deno.core.opAsync("write", rid, data);
}

function close(rid) {
  Deno.core.opSync("close", rid);
}

async function serve(rid) {
  try {
    while (true) {
      await read(rid, requestBuf);
      await write(rid, responseBuf);
    }
  } catch (e) {
    if (
      !e.message.includes("Broken pipe") &&
      !e.message.includes("Connection reset by peer")
    ) {
      throw e;
    }
  }
  close(rid);
}

async function main() {
  Deno.core.ops();
  Deno.core.registerErrorClass("Error", Error);

  const listenerRid = listen();
  Deno.core.print(`http_bench_json_ops listening on http://127.0.0.1:4544/\n`);

  while (true) {
    const rid = await accept(listenerRid);
    serve(rid);
  }
}

main();
