// This is not a real HTTP server. We read blindly one time into 'requestBuf',
// then write this fixed 'responseBuf'. The point of this benchmark is to
// exercise the event loop in a simple yet semi-realistic way.

/** This structure is used to collect all ops
 * and assign ids to them after we get them
 * from Rust.
 *
 * @type {Map<string, HttpOp>}
 */
const opRegistry = new Map();

class HttpOp {
  constructor(name) {
    if (typeof opRegistry.get(name) !== "undefined") {
      throw new Error(`Duplicate op: ${name}`);
    }

    this.name = name;
    this.opId = 0;
    opRegistry.set(name, this);
  }

  setOpId(opId) {
    this.opId = opId;
  }

  static handleAsyncMsgFromRust(opId, buf) {
    const record = recordFromBuf(buf);
    const { promiseId } = record;
    const p = promiseMap.get(promiseId);
    promiseMap.delete(promiseId);
    p.resolve(record);
  }

  static sendSync(opId, arg, zeroCopy) {
    const buf = send(0, opId, arg, zeroCopy);
    return recordFromBuf(buf);
  }

  static sendAsync(opId, arg, zeroCopy = null) {
    const promiseId = nextPromiseId++;
    const p = createResolvable();
    promiseMap.set(promiseId, p);
    send(promiseId, opId, arg, zeroCopy);
    return p;
  }

  /** Returns i32 number */
  sendSync(arg, zeroCopy = null) {
    const res = HttpOp.sendSync(this.opId, arg, zeroCopy);
    return res.result;
  }

  /** Returns Promise<number> */
  async sendAsync(arg, zeroCopy = null) {
    const res = await HttpOp.sendAsync(this.opId, arg, zeroCopy);
    return res.result;
  }
}

const OP_LISTEN = new HttpOp("listen");
const OP_ACCEPT = new HttpOp("accept");
const OP_READ = new HttpOp("read");
const OP_WRITE = new HttpOp("write");
const OP_CLOSE = new HttpOp("close");

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

function recordFromBuf(buf) {
  assert(buf.byteLength === 3 * 4);
  const buf32 = new Int32Array(buf.buffer, buf.byteOffset, buf.byteLength / 4);
  return {
    promiseId: buf32[0],
    arg: buf32[1],
    result: buf32[2]
  };
}

/** Listens on 0.0.0.0:4500, returns rid. */
function listen() {
  return OP_LISTEN.sendSync(-1);
}

/** Accepts a connection, returns rid. */
async function accept(rid) {
  return await OP_ACCEPT.sendAsync(rid);
}

/**
 * Reads a packet from the rid, presumably an http request. data is ignored.
 * Returns bytes read.
 */
async function read(rid, data) {
  return await OP_READ.sendAsync(rid, data);
}

/** Writes a fixed HTTP response to the socket rid. Returns bytes written. */
async function write(rid, data) {
  return await OP_WRITE.sendAsync(rid, data);
}

function close(rid) {
  return OP_CLOSE.sendSync(rid);
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
  Deno.core.setAsyncHandler(HttpOp.handleAsyncMsgFromRust);
  // Initialize ops by getting their ids from Rust
  // and assign id for each of our ops.
  const opsMap = Deno.core.getOps();
  for (const [name, opId] of Object.entries(opsMap)) {
    const op = opRegistry.get(name);

    if (!op) {
      throw new Error(`Unknown op: ${name}`);
    }

    op.setOpId(opId);
  }

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
