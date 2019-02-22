// This is not a real HTTP server. We read blindly one time into 'requestBuf',
// then write this fixed 'responseBuf'. The point of this benchmark is to
// exercise the event loop in a simple yet semi-realistic way.
const shared32 = new Int32Array(libdeno.shared);

const INDEX_NUM_RECORDS = 0;
const INDEX_RECORDS = 1;
const RECORD_OFFSET_PROMISE_ID = 0;
const RECORD_OFFSET_OP = 1;
const RECORD_OFFSET_ARG = 2;
const RECORD_OFFSET_RESULT = 3;
const RECORD_SIZE = 4;
const OP_LISTEN = 1;
const OP_ACCEPT = 2;
const OP_READ = 3;
const OP_WRITE = 4;
const OP_CLOSE = 5;

const NUM_RECORDS = (shared32.length - INDEX_RECORDS) / RECORD_SIZE;
if (NUM_RECORDS != 100) {
  throw Error("expected 100 entries");
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
function sendAsync(op, arg, zeroCopyData) {
  const id = nextPromiseId++;
  const p = createResolvable();
  shared32[INDEX_NUM_RECORDS] = 1;
  setRecord(0, RECORD_OFFSET_PROMISE_ID, id);
  setRecord(0, RECORD_OFFSET_OP, op);
  setRecord(0, RECORD_OFFSET_ARG, arg);
  setRecord(0, RECORD_OFFSET_RESULT, -1);
  promiseMap.set(id, p);
  libdeno.send(null, zeroCopyData);
  return p;
}

/** Returns u32 number */
function sendSync(op, arg) {
  shared32[INDEX_NUM_RECORDS] = 1;
  setRecord(0, RECORD_OFFSET_PROMISE_ID, 0);
  setRecord(0, RECORD_OFFSET_OP, op);
  setRecord(0, RECORD_OFFSET_ARG, arg);
  setRecord(0, RECORD_OFFSET_RESULT, -1);
  libdeno.send();
  return getRecord(0, RECORD_OFFSET_RESULT);
}

function setRecord(i, off, value) {
  if (i >= NUM_RECORDS) {
    throw Error("out of range");
  }
  shared32[INDEX_RECORDS + RECORD_SIZE * i + off] = value;
}

function getRecord(i, off) {
  if (i >= NUM_RECORDS) {
    throw Error("out of range");
  }
  return shared32[INDEX_RECORDS + RECORD_SIZE * i + off];
}

function handleAsyncMsgFromRust() {
  for (let i = 0; i < shared32[INDEX_NUM_RECORDS]; i++) {
    let id = getRecord(i, RECORD_OFFSET_PROMISE_ID);
    const p = promiseMap.get(id);
    promiseMap.delete(id);
    p.resolve(getRecord(i, RECORD_OFFSET_RESULT));
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

  libdeno.print("http_bench.js start");

  const listener_rid = listen();
  libdeno.print(`listening http://127.0.0.1:4544/ rid = ${listener_rid}`);
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
