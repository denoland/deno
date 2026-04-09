// Copyright 2018-2026 the Deno authors. MIT license.

// Interfaces 100% copied from Go.
// Documentation liberally lifted from them too.
// Thank you! We love Go! <3

import { core, primordials } from "ext:core/mod.js";
import { op_set_raw } from "ext:core/ops";
const {
  Uint8Array,
  ArrayPrototypePush,
  Promise,
  Symbol,
  TypedArrayPrototypeSubarray,
  TypedArrayPrototypeSet,
  TypedArrayPrototypeGetByteLength,
} = primordials;

import {
  readableStreamForRid,
  writableStreamForRid,
} from "ext:deno_web/06_streams.js";

// Seek whence values.
// https://golang.org/pkg/io/#pkg-constants
const SeekMode = {
  0: "Start",
  1: "Current",
  2: "End",

  Start: 0,
  Current: 1,
  End: 2,
};

function readSync(rid, buffer) {
  if (buffer.length === 0) return 0;
  const nread = core.readSync(rid, buffer);
  return nread === 0 ? null : nread;
}

async function read(rid, buffer) {
  if (buffer.length === 0) return 0;
  const nread = await core.read(rid, buffer);
  return nread === 0 ? null : nread;
}

function writeSync(rid, data) {
  return core.writeSync(rid, data);
}

function write(rid, data) {
  return core.write(rid, data);
}

const READ_PER_ITER = 64 * 1024; // 64kb

async function readAll(r) {
  const buffers = [];

  while (true) {
    const buf = new Uint8Array(READ_PER_ITER);
    const read = await r.read(buf);
    if (typeof read == "number") {
      ArrayPrototypePush(buffers, TypedArrayPrototypeSubarray(buf, 0, read));
    } else {
      break;
    }
  }

  return concatBuffers(buffers);
}

function readAllSync(r) {
  const buffers = [];

  while (true) {
    const buf = new Uint8Array(READ_PER_ITER);
    const read = r.readSync(buf);
    if (typeof read == "number") {
      ArrayPrototypePush(buffers, TypedArrayPrototypeSubarray(buf, 0, read));
    } else {
      break;
    }
  }

  return concatBuffers(buffers);
}

function concatBuffers(buffers) {
  let totalLen = 0;
  for (let i = 0; i < buffers.length; ++i) {
    totalLen += TypedArrayPrototypeGetByteLength(buffers[i]);
  }

  const contents = new Uint8Array(totalLen);

  let n = 0;
  for (let i = 0; i < buffers.length; ++i) {
    const buf = buffers[i];
    TypedArrayPrototypeSet(contents, buf, n);
    n += TypedArrayPrototypeGetByteLength(buf);
  }

  return contents;
}

const STDIN_RID = 0;
const STDOUT_RID = 1;
const STDERR_RID = 2;

const REF = Symbol("REF");
const UNREF = Symbol("UNREF");

// Module-level Node stream references. Set by __setNodeStreams() during
// Node bootstrap so that Deno.stdin/stdout/stderr delegate to the Node
// process.stdin/stdout/stderr streams (which own the TTY handles).
let nodeStdin = null;
let nodeStdout = null;
let nodeStderr = null;

// Cached Web stream wrappers for the Node streams.
let nodeStdinReadable = null;
let nodeStdoutWritable = null;
let nodeStderrWritable = null;

// Called from Node bootstrap to wire up delegation.
function __setNodeStreams(stdinStream, stdoutStream, stderrStream) {
  nodeStdin = stdinStream;
  nodeStdout = stdoutStream;
  nodeStderr = stderrStream;
  // Invalidate any cached Web streams.
  nodeStdinReadable = null;
  nodeStdoutWritable = null;
  nodeStderrWritable = null;
}

// Helper: wrap a Node.js Readable as a Web ReadableStream.
function nodeReadableToWeb(nodeStream) {
  return new ReadableStream({
    start(controller) {
      nodeStream.on("data", (chunk) => {
        const bytes = chunk instanceof Uint8Array
          ? chunk
          : new Uint8Array(chunk);
        controller.enqueue(bytes);
      });
      nodeStream.on("end", () => controller.close());
      nodeStream.on("error", (err) => controller.error(err));
    },
    cancel() {
      nodeStream.destroy();
    },
  });
}

// Helper: wrap a Node.js Writable as a Web WritableStream.
function nodeWritableToWeb(nodeStream) {
  return new WritableStream({
    write(chunk) {
      return new Promise((resolve, reject) => {
        nodeStream.write(chunk, (err) => {
          if (err) reject(err);
          else resolve();
        });
      });
    },
    close() {
      return new Promise((resolve) => {
        nodeStream.end(resolve);
      });
    },
    abort(reason) {
      nodeStream.destroy(reason);
    },
  });
}

// Helper: read from a Node Readable into a Uint8Array buffer.
function readFromNodeStream(ns, p) {
  if (p.length === 0) return Promise.resolve(0);
  // Try a synchronous read first (data may already be buffered).
  const chunk = ns.read(p.length);
  if (chunk !== null) {
    const bytes = chunk instanceof Uint8Array ? chunk : new Uint8Array(chunk);
    TypedArrayPrototypeSet(p, bytes, 0);
    return Promise.resolve(bytes.length);
  }
  // Wait for data or end.
  return new Promise((resolve, reject) => {
    const onReadable = () => {
      const chunk = ns.read(p.length);
      if (chunk !== null) {
        const bytes = chunk instanceof Uint8Array
          ? chunk
          : new Uint8Array(chunk);
        TypedArrayPrototypeSet(p, bytes, 0);
        cleanup();
        resolve(bytes.length);
      }
    };
    const onEnd = () => {
      cleanup();
      resolve(null);
    };
    const onError = (err) => {
      cleanup();
      reject(err);
    };
    const cleanup = () => {
      ns.removeListener("readable", onReadable);
      ns.removeListener("end", onEnd);
      ns.removeListener("error", onError);
    };
    ns.on("readable", onReadable);
    ns.on("end", onEnd);
    ns.on("error", onError);
  });
}

class Stdin {
  #rid = STDIN_RID;
  #ref = true;
  #readable;
  #opPromise;

  constructor() {
  }

  get rid() {
    return this.#rid;
  }

  read(p) {
    if (nodeStdin) {
      return readFromNodeStream(nodeStdin, p);
    }
    return this.#readFromDeno(p);
  }

  async #readFromDeno(p) {
    if (p.length === 0) return 0;
    this.#opPromise = core.read(this.#rid, p);
    if (!this.#ref) {
      core.unrefOpPromise(this.#opPromise);
    }
    const nread = await this.#opPromise;
    return nread === 0 ? null : nread;
  }

  readSync(p) {
    return readSync(this.#rid, p);
  }

  close() {
    if (nodeStdin) {
      nodeStdin.destroy();
      return;
    }
    core.tryClose(this.#rid);
  }

  get readable() {
    if (nodeStdin) {
      if (nodeStdinReadable === null) {
        nodeStdinReadable = nodeReadableToWeb(nodeStdin);
      }
      return nodeStdinReadable;
    }
    if (this.#readable === undefined) {
      this.#readable = readableStreamForRid(this.#rid, false);
    }
    return this.#readable;
  }

  setRaw(mode, options = { __proto__: null }) {
    if (nodeStdin && nodeStdin.setRawMode) {
      nodeStdin.setRawMode(mode);
      return;
    }
    const cbreak = !!(options.cbreak ?? false);
    op_set_raw(this.#rid, mode, cbreak);
  }

  isTerminal() {
    if (nodeStdin) {
      return !!nodeStdin.isTTY;
    }
    return core.isTerminal(this.#rid);
  }

  [REF]() {
    this.#ref = true;
    if (nodeStdin && nodeStdin._handle?.ref) {
      nodeStdin._handle.ref();
      return;
    }
    if (this.#opPromise) {
      core.refOpPromise(this.#opPromise);
    }
  }

  [UNREF]() {
    this.#ref = false;
    if (nodeStdin && nodeStdin._handle?.unref) {
      nodeStdin._handle.unref();
      return;
    }
    if (this.#opPromise) {
      core.unrefOpPromise(this.#opPromise);
    }
  }
}

class Stdout {
  #rid = STDOUT_RID;
  #writable;

  constructor() {
  }

  get rid() {
    return this.#rid;
  }

  write(p) {
    if (nodeStdout) {
      return new Promise((resolve, reject) => {
        nodeStdout.write(p, (err) => {
          if (err) reject(err);
          else resolve(p.length);
        });
      });
    }
    return write(this.#rid, p);
  }

  writeSync(p) {
    if (nodeStdout) {
      nodeStdout.write(p);
      return p.length;
    }
    return writeSync(this.#rid, p);
  }

  close() {
    if (nodeStdout) {
      nodeStdout.destroy();
      return;
    }
    core.close(this.#rid);
  }

  get writable() {
    if (nodeStdout) {
      if (nodeStdoutWritable === null) {
        nodeStdoutWritable = nodeWritableToWeb(nodeStdout);
      }
      return nodeStdoutWritable;
    }
    if (this.#writable === undefined) {
      this.#writable = writableStreamForRid(this.#rid);
    }
    return this.#writable;
  }

  isTerminal() {
    if (nodeStdout) {
      return !!nodeStdout.isTTY;
    }
    return core.isTerminal(this.#rid);
  }
}

class Stderr {
  #rid = STDERR_RID;
  #writable;

  constructor() {
  }

  get rid() {
    return this.#rid;
  }

  write(p) {
    if (nodeStderr) {
      return new Promise((resolve, reject) => {
        nodeStderr.write(p, (err) => {
          if (err) reject(err);
          else resolve(p.length);
        });
      });
    }
    return write(this.#rid, p);
  }

  writeSync(p) {
    if (nodeStderr) {
      nodeStderr.write(p);
      return p.length;
    }
    return writeSync(this.#rid, p);
  }

  close() {
    if (nodeStderr) {
      nodeStderr.destroy();
      return;
    }
    core.close(this.#rid);
  }

  get writable() {
    if (nodeStderr) {
      if (nodeStderrWritable === null) {
        nodeStderrWritable = nodeWritableToWeb(nodeStderr);
      }
      return nodeStderrWritable;
    }
    if (this.#writable === undefined) {
      this.#writable = writableStreamForRid(this.#rid);
    }
    return this.#writable;
  }

  isTerminal() {
    if (nodeStderr) {
      return !!nodeStderr.isTTY;
    }
    return core.isTerminal(this.#rid);
  }
}

const stdin = new Stdin();
const stdout = new Stdout();
const stderr = new Stderr();

export {
  __setNodeStreams,
  read,
  readAll,
  readAllSync,
  readSync,
  REF,
  SeekMode,
  Stderr,
  stderr,
  STDERR_RID,
  stdin,
  STDIN_RID,
  Stdout,
  stdout,
  STDOUT_RID,
  UNREF,
  write,
  writeSync,
};
