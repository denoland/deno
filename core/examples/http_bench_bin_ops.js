// This is not a real HTTP server. We read blindly one time into 'requestBuf',
// then write this fixed 'responseBuf'. The point of this benchmark is to
// exercise the event loop in a simple yet semi-realistic way.
const requestBuf = new Uint8Array(64 * 1024);
const responseBuf = new Uint8Array(
  "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n"
    .split("")
    .map((c) => c.charCodeAt(0)),
);
const promiseMap = new Map();
let nextPromiseId = 1;

function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function createResolvable() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  promise.resolve = resolve;
  promise.reject = reject;
  return promise;
}

const scratch32 = new Int32Array(3);
const scratchBytes = new Uint8Array(
  scratch32.buffer,
  scratch32.byteOffset,
  scratch32.byteLength,
);
assert(scratchBytes.byteLength === 3 * 4);

function send(promiseId, opId, rid, ...zeroCopy) {
  scratch32[0] = promiseId;
  scratch32[1] = rid;
  scratch32[2] = -1;
  return Deno.core.dispatch(opId, scratchBytes, ...zeroCopy);
}

/** Returns Promise<number> */
function sendAsync(opId, rid, ...zeroCopy) {
  const promiseId = nextPromiseId++;
  const p = createResolvable();
  const buf = send(promiseId, opId, rid, ...zeroCopy);
  if (buf) {
    const record = recordFromBuf(buf);
    // Sync result.
    p.resolve(record.result);
  } else {
    // Async result.
    promiseMap.set(promiseId, p);
  }
  return p;
}

/** Returns i32 number */
function sendSync(opId, rid) {
  const buf = send(0, opId, rid);
  const record = recordFromBuf(buf);
  return record[2];
}

function recordFromBuf(buf) {
  assert(buf.byteLength === 3 * 4);
  return new Int32Array(buf.buffer, buf.byteOffset, buf.byteLength / 4);
}

function handleAsyncMsgFromRust(buf) {
  const record = recordFromBuf(buf);
  const p = promiseMap.get(record[0]);
  promiseMap.delete(record[0]);
  p.resolve(record[2]);
}

/** Listens on 0.0.0.0:4500, returns rid. */
function listen() {
  return sendSync(ops["listen"], -1);
}

/** Accepts a connection, returns rid. */
function accept(rid) {
  return sendAsync(ops["accept"], rid);
}

/**
 * Reads a packet from the rid, presumably an http request. data is ignored.
 * Returns bytes read.
 */
function read(rid, data) {
  return sendAsync(ops["read"], rid, data);
}

/** Writes a fixed HTTP response to the socket rid. Returns bytes written. */
function write(rid, data) {
  return sendAsync(ops["write"], rid, data);
}

function close(rid) {
  return sendSync(ops["close"], rid);
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

let ops;

async function main() {
  ops = Deno.core.ops();
  for (const opName in ops) {
    Deno.core.setAsyncHandler(ops[opName], handleAsyncMsgFromRust);
  }

  const listenerRid = listen();
  Deno.core.print(`http_bench_bin_ops listening on http://127.0.0.1:4544/\n`);

  for (;;) {
    const rid = await accept(listenerRid);
    // Deno.core.print(`accepted ${rid}`);
    if (rid < 0) {
      Deno.core.print(`accept error ${rid}`);
      return;
    }
    serve(rid);
  }
}

main();
