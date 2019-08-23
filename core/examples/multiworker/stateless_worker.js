const promiseMap = new Map();
let nextPromiseId = 1;

const opNamespace = Deno.ops.statelessWorker;

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

function handleAsyncMsgFromRust(buf) {
  const record = recordFromBuf(buf);
  const { promiseId, result } = record;
  const p = promiseMap.get(promiseId);
  promiseMap.delete(promiseId);
  p.resolve(result);
}

let acceptOpId;
opNamespace.accept = id => {
  acceptOpId = id;
  Deno.core.setAsyncHandler(id, handleAsyncMsgFromRust);
};
/** Accepts a connection, returns rid. */
async function accept() {
  return await sendAsync(acceptOpId, 0);
}

let closeOpId;
opNamespace.close = id => {
  closeOpId = id;
  Deno.core.setAsyncHandler(id, handleAsyncMsgFromRust);
};
function close(rid) {
  return sendSync(closeOpId, rid);
}

let readOpId;
opNamespace.read = id => {
  readOpId = id;
  Deno.core.setAsyncHandler(id, handleAsyncMsgFromRust);
};
async function read(rid, data) {
  return await sendAsync(readOpId, rid, data);
}

let writeOpId;
opNamespace.write = id => {
  writeOpId = id;
  Deno.core.setAsyncHandler(id, handleAsyncMsgFromRust);
};
async function write(rid, data) {
  return await sendAsync(writeOpId, rid, data);
}
