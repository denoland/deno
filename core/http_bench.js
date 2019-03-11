// This is not a real HTTP server. We read blindly one time into 'requestBuf',
// then write this fixed 'responseBuf'. The point of this benchmark is to
// exercise the event loop in a simple yet semi-realistic way.
const OP_LISTEN = 1;
const OP_ACCEPT = 2;
const OP_READ = 3;
const OP_WRITE = 4;
const OP_CLOSE = 5;
const INDEX_START = 0;
const INDEX_END = 1;
const NUM_RECORDS = 128;
const RECORD_SIZE = 4;

const shared32 = new Int32Array(libdeno.shared);

function idx(i, off) {
  return 2 + i * RECORD_SIZE + off;
}

function recordsPush(promiseId, opId, arg, result) {
  let i = shared32[INDEX_END];
  if (i >= NUM_RECORDS) {
    return false;
  }
  shared32[idx(i, 0)] = promiseId;
  shared32[idx(i, 1)] = opId;
  shared32[idx(i, 2)] = arg;
  shared32[idx(i, 3)] = result;
  shared32[INDEX_END]++;
  return true;
}

function recordsShift() {
  if (shared32[INDEX_START] == shared32[INDEX_END]) {
    return null;
  }
  const i = shared32[INDEX_START];
  const record = {
    promiseId: shared32[idx(i, 0)],
    opId: shared32[idx(i, 1)],
    arg: shared32[idx(i, 2)],
    result: shared32[idx(i, 3)]
  };
  shared32[INDEX_START]++;
  return record;
}

function recordsReset() {
  shared32[INDEX_START] = 0;
  shared32[INDEX_END] = 0;
}

function recordsSize() {
  return shared32[INDEX_END] - shared32[INDEX_START];
}

const requestBuf = new Uint8Array(64 * 1024);
const responseBuf = new Uint8Array(
  "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n"
    .split("")
    .map(c => c.charCodeAt(0))
);

const promiseMap = new Map();
let nextPromiseId = 1;

function createResolvable() {
  let methods;
  const promise = new Promise((resolve, reject) => {
    methods = { resolve, reject };
  });
  return Object.assign(promise, methods);
}

/** Returns Promise<number> */
function sendAsync(opId, arg, zeroCopyData) {
  const promiseId = nextPromiseId++;
  const p = createResolvable();
  recordsReset();
  recordsPush(promiseId, opId, arg, -1);
  promiseMap.set(promiseId, p);
  libdeno.send(null, zeroCopyData);
  return p;
}

/** Returns u32 number */
function sendSync(opId, arg) {
  recordsReset();
  recordsPush(0, opId, arg, -1);
  libdeno.send();
  if (recordsSize() != 1) {
    throw Error("Expected sharedSimple to have size 1");
  }
  let { result } = recordsShift();
  return result;
}

function handleAsyncMsgFromRust() {
  while (recordsSize() > 0) {
    const { promiseId, result } = recordsShift();
    const p = promiseMap.get(promiseId);
    promiseMap.delete(promiseId);
    p.resolve(result);
  }
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
  libdeno.recv(handleAsyncMsgFromRust);

  libdeno.print("http_bench.js start\n");

  const listenerRid = listen();
  libdeno.print(`listening http://127.0.0.1:4544/ rid = ${listenerRid}`);
  while (true) {
    const rid = await accept(listenerRid);
    // libdeno.print(`accepted ${rid}`);
    if (rid < 0) {
      libdeno.print(`accept error ${rid}`);
      return;
    }
    serve(rid);
  }
}

main();
