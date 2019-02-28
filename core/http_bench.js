// This is not a real HTTP server. We read blindly one time into 'requestBuf',
// then write this fixed 'responseBuf'. The point of this benchmark is to
// exercise the event loop in a simple yet semi-realistic way.
const OP_LISTEN = 1;
const OP_ACCEPT = 2;
const OP_READ = 3;
const OP_WRITE = 4;
const OP_CLOSE = 5;

const INDEX_LEN = 0;
const NUM_RECORDS = 128;
const RECORD_SIZE = 4;
const shared32 = new Int32Array(libdeno.shared);
const global = this;

if (!global["Deno"]) {
  global["Deno"] = {};
}

function idx(i, off) {
  return 1 + i * RECORD_SIZE + off;
}

function recordsPush(promiseId, opId, arg, result) {
  if (shared32[INDEX_LEN] >= NUM_RECORDS) {
    return false;
  }
  const i = shared32[INDEX_LEN]++;
  shared32[idx(i, 0)] = promiseId;
  shared32[idx(i, 1)] = opId;
  shared32[idx(i, 2)] = arg;
  shared32[idx(i, 3)] = result;
  return true;
}

function recordsPop() {
  if (shared32[INDEX_LEN] == 0) {
    return null;
  }
  const i = --shared32[INDEX_LEN];
  return {
    promiseId: shared32[idx(i, 0)],
    opId: shared32[idx(i, 1)],
    arg: shared32[idx(i, 2)],
    result: shared32[idx(i, 3)]
  };
}

function recordsReset() {
  shared32[INDEX_LEN] = 0;
}

function recordsSize() {
  return shared32[INDEX_LEN];
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
  let { result } = recordsPop();
  return result;
}

function handleAsyncMsgFromRust() {
  while (recordsSize() > 0) {
    const { promiseId, result } = recordsPop();
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

  const listener_rid = listen();
  libdeno.print(`listening http://127.0.0.1:4544/ rid = ${listener_rid}\n`);
  while (true) {
    const rid = await accept(listener_rid);
    // libdeno.print(`accepted ${rid}`);
    if (rid < 0) {
      libdeno.print(`accept error ${rid}`);
      return;
    }
    serve(rid);
  }
}

main();
