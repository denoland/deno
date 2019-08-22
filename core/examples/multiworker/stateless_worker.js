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

/** Returns Promise<number> */
function sendAsync(opId, data, zeroCopy = null) {
  const promiseId = nextPromiseId++;
  const p = createResolvable();
  promiseMap.set(promiseId, p);
  const dataFinal = new Uint8Array(data.byteLength + 4);
  new DataView(dataFinal.buffer, 0, 4).setInt32(0, promiseId, true);
  dataFinal.set(
    new Uint8Array(data.buffer, data.byteOffset, data.byteLength),
    4
  );
  Deno.core.dispatch(opId, dataFinal, zeroCopy);
  return p;
}

function handleAsyncMsgFromRust(opId, buf) {
  const promiseId = new Int32Array(
    buf.buffer,
    buf.byteOffset,
    buf.byteLength
  )[0];
  const result = new Uint8Array(
    buf.buffer,
    buf.byteOffset + 4,
    buf.byteLength - 4
  );
  const p = promiseMap.get(promiseId);
  promiseMap.delete(promiseId);
  p.resolve(result);
}

let acceptOpId;
opNamespace.accept = id => {
  acceptOpId = id;
};
/** Accepts a connection, returns rid. */
async function accept() {
  const response = await sendAsync(acceptOpId, new Int32Array([]));
  const responseDataView = new Int32Array(
    response.buffer,
    response.byteOffset,
    response.byteLength
  );
  return responseDataView[0];
}

let closeOpId;
opNamespace.close = id => {
  closeOpId = id;
};
function close(rid) {
  const response = Deno.core.dispatch(closeOpId, new Int32Array([rid]));
  const responseDataView = new Int32Array(
    response.buffer,
    response.byteOffset,
    response.byteLength
  );
  return responseDataView[0];
}

let readOpId;
opNamespace.read = id => {
  readOpId = id;
};
async function read(rid, data) {
  const response = await sendAsync(readOpId, new Int32Array([rid]), data);
  const responseDataView = new Int32Array(
    response.buffer,
    response.byteOffset,
    response.byteLength
  );
  return responseDataView[0];
}

let writeOpId;
opNamespace.write = id => {
  writeOpId = id;
};
async function write(rid, data) {
  const response = await sendAsync(writeOpId, new Int32Array([rid]), data);
  const responseDataView = new Int32Array(
    response.buffer,
    response.byteOffset,
    response.byteLength
  );
  return responseDataView[0];
}
