// This is not a real HTTP server. We read blindly one time into 'requestBuf',
// then write this fixed 'responseBuf'. The point of this benchmark is to
// exercise the event loop in a simple yet semi-realistic way.
const OP_LISTEN = 1;
const OP_ACCEPT = 2;
const OP_READ = 3;
const OP_WRITE = 4;
const OP_CLOSE = 5;
const requestBuf = new Uint8Array(64 * 1024);
const responseBuf = new Uint8Array(
  "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n"
    .split("")
    .map(c => c.charCodeAt(0))
);
const promiseMap = new Map();
let nextPromiseId = 1;

function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

function createResolvable() {
  let methods;
  const promise = new Promise((resolve, reject) => {
    methods = { resolve, reject };
  });
  return Object.assign(promise, methods);
}

const scratch32 = new Int32Array(3);
const scratchBytes = new Uint8Array(
  scratch32.buffer,
  scratch32.byteOffset,
  scratch32.byteLength
);
assert(scratchBytes.byteLength === 3 * 4);

function send(promiseId, opId, arg, zeroCopy = null) {
  scratch32[0] = promiseId;
  scratch32[1] = arg;
  scratch32[2] = -1;
  return Deno.core.dispatch(opId, scratchBytes, zeroCopy);
}

/** Returns Promise<number> */
function sendAsync(opId, arg, zeroCopy = null) {
  const promiseId = nextPromiseId++;
  const p = createResolvable();
  promiseMap.set(promiseId, p);
  send(promiseId, opId, arg, zeroCopy);
  return p;
}

function recordFromBuf(buf) {
  assert(buf.byteLength === 3 * 4);
  const buf32 = new Int32Array(buf.buffer, buf.byteOffset, buf.byteLength / 4);
  return {
    promiseId: buf32[0],
    arg: buf32[1],
    result: buf32[2]
  };
}

/** Returns i32 number */
function sendSync(opId, arg) {
  const buf = send(0, opId, arg);
  const record = recordFromBuf(buf);
  return record.result;
}

function handleAsyncMsgFromRust(opId, buf) {
  const record = recordFromBuf(buf);
  const { promiseId, result } = record;
  const p = promiseMap.get(promiseId);
  promiseMap.delete(promiseId);
  p.resolve(result);
}

/** Listens on 0.0.0.0:4500, returns rid. */
function listen() {
  return sendSync(OP_LISTEN, -1);
}

/** Accepts a connection, returns rid. */
async function accept(rid) {
  return await sendAsync(OP_ACCEPT, rid);
}

/**
 * Reads a packet from the rid, presumably an http request. data is ignored.
 * Returns bytes read.
 */
async function read(rid, data) {
  return await sendAsync(OP_READ, rid, data);
}

/** Writes a fixed HTTP response to the socket rid. Returns bytes written. */
async function write(rid, data) {
  return await sendAsync(OP_WRITE, rid, data);
}

function close(rid) {
  return sendSync(OP_CLOSE, rid);
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
  Deno.core.setAsyncHandler(handleAsyncMsgFromRust);

  Deno.core.print("http_bench.js start\n");

  const listenerRid = listen();
  Deno.core.print(`listening http://127.0.0.1:4544/ rid = ${listenerRid}\n`);
  while (true) {
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
