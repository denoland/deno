// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This is not a real HTTP server. We read blindly one time into 'requestBuf',
// then write this fixed 'responseBuf'. The point of this benchmark is to
// exercise the event loop in a simple yet semi-realistic way.
Deno.core.initializeAsyncOps();

const { ops } = Deno.core;

const requestBuf = new Uint8Array(64 * 1024);
const responseBuf = new Uint8Array(
  "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n"
    .split("")
    .map((c) => c.charCodeAt(0)),
);

/** Listens on 0.0.0.0:4570, returns rid. */
function listen() {
  return ops.op_listen();
}

/** Accepts a connection, returns rid. */
function accept(serverRid) {
  return ops.op_accept(serverRid);
}

function read(serverRid, buf) {
  return ops.op_read_socket(serverRid, buf);
}

async function serve(rid) {
  try {
    while (true) {
      await read(rid, requestBuf);
      if (!ops.op_try_write(rid, responseBuf)) {
        await Deno.core.writeAll(rid, responseBuf);
      }
    }
  } catch {
    // pass
  }
  Deno.core.close(rid);
}

async function main() {
  const listenerRid = listen();
  Deno.core.print(`http_bench_ops listening on http://127.0.0.1:4570/\n`);

  while (true) {
    const rid = await accept(listenerRid);
    serve(rid);
  }
}

main();
