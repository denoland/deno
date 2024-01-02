// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Interfaces 100% copied from Go.
// Documentation liberally lifted from them too.
// Thank you! We love Go! <3

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
import {
  readableStreamForRid,
  writableStreamForRid,
} from "ext:deno_web/06_streams.js";
const {
  Uint8Array,
  ArrayPrototypePush,
  MathMin,
  TypedArrayPrototypeSubarray,
  TypedArrayPrototypeSet,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
} = primordials;

const DEFAULT_BUFFER_SIZE = 32 * 1024;
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

async function copy(
  src,
  dst,
  options,
) {
  let n = 0;
  const bufSize = options?.bufSize ?? DEFAULT_BUFFER_SIZE;
  const b = new Uint8Array(bufSize);
  let gotEOF = false;
  while (gotEOF === false) {
    const result = await src.read(b);
    if (result === null) {
      gotEOF = true;
    } else {
      let nwritten = 0;
      while (nwritten < result) {
        nwritten += await dst.write(
          TypedArrayPrototypeSubarray(b, nwritten, result),
        );
      }
      n += nwritten;
    }
  }
  return n;
}

async function* iter(
  r,
  options,
) {
  const bufSize = options?.bufSize ?? DEFAULT_BUFFER_SIZE;
  const b = new Uint8Array(bufSize);
  while (true) {
    const result = await r.read(b);
    if (result === null) {
      break;
    }

    yield TypedArrayPrototypeSubarray(b, 0, result);
  }
}

function* iterSync(
  r,
  options,
) {
  const bufSize = options?.bufSize ?? DEFAULT_BUFFER_SIZE;
  const b = new Uint8Array(bufSize);
  while (true) {
    const result = r.readSync(b);
    if (result === null) {
      break;
    }

    yield TypedArrayPrototypeSubarray(b, 0, result);
  }
}

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

function readAll(r) {
  return readAllInner(r);
}
async function readAllInner(r, options) {
  const buffers = [];
  const signal = options?.signal ?? null;
  while (true) {
    signal?.throwIfAborted();
    const buf = new Uint8Array(READ_PER_ITER);
    const read = await r.read(buf);
    if (typeof read == "number") {
      ArrayPrototypePush(
        buffers,
        new Uint8Array(TypedArrayPrototypeGetBuffer(buf), 0, read),
      );
    } else {
      break;
    }
  }
  signal?.throwIfAborted();

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

function readAllSyncSized(r, size) {
  const buf = new Uint8Array(size + 1); // 1B to detect extended files
  let cursor = 0;

  while (cursor < size) {
    const sliceEnd = MathMin(size + 1, cursor + READ_PER_ITER);
    const slice = TypedArrayPrototypeSubarray(buf, cursor, sliceEnd);
    const read = r.readSync(slice);
    if (typeof read == "number") {
      cursor += read;
    } else {
      break;
    }
  }

  // Handle truncated or extended files during read
  if (cursor > size) {
    // Read remaining and concat
    return concatBuffers([buf, readAllSync(r)]);
  } else { // cursor == size
    return TypedArrayPrototypeSubarray(buf, 0, cursor);
  }
}

async function readAllInnerSized(r, size, options) {
  const buf = new Uint8Array(size + 1); // 1B to detect extended files
  let cursor = 0;
  const signal = options?.signal ?? null;
  while (cursor < size) {
    signal?.throwIfAborted();
    const sliceEnd = MathMin(size + 1, cursor + READ_PER_ITER);
    const slice = TypedArrayPrototypeSubarray(buf, cursor, sliceEnd);
    const read = await r.read(slice);
    if (typeof read == "number") {
      cursor += read;
    } else {
      break;
    }
  }
  signal?.throwIfAborted();

  // Handle truncated or extended files during read
  if (cursor > size) {
    // Read remaining and concat
    return concatBuffers([buf, await readAllInner(r, options)]);
  } else {
    return TypedArrayPrototypeSubarray(buf, 0, cursor);
  }
}

class Stdin {
  #readable;

  constructor() {
  }

  get rid() {
    return 0;
  }

  read(p) {
    return read(this.rid, p);
  }

  readSync(p) {
    return readSync(this.rid, p);
  }

  close() {
    core.tryClose(this.rid);
  }

  get readable() {
    if (this.#readable === undefined) {
      this.#readable = readableStreamForRid(this.rid);
    }
    return this.#readable;
  }

  setRaw(mode, options = {}) {
    const cbreak = !!(options.cbreak ?? false);
    ops.op_stdin_set_raw(mode, cbreak);
  }
}

class Stdout {
  #writable;

  constructor() {
  }

  get rid() {
    return 1;
  }

  write(p) {
    return write(this.rid, p);
  }

  writeSync(p) {
    return writeSync(this.rid, p);
  }

  close() {
    core.close(this.rid);
  }

  get writable() {
    if (this.#writable === undefined) {
      this.#writable = writableStreamForRid(this.rid);
    }
    return this.#writable;
  }
}

class Stderr {
  #writable;

  constructor() {
  }

  get rid() {
    return 2;
  }

  write(p) {
    return write(this.rid, p);
  }

  writeSync(p) {
    return writeSync(this.rid, p);
  }

  close() {
    core.close(this.rid);
  }

  get writable() {
    if (this.#writable === undefined) {
      this.#writable = writableStreamForRid(this.rid);
    }
    return this.#writable;
  }
}

const stdin = new Stdin();
const stdout = new Stdout();
const stderr = new Stderr();

export {
  copy,
  iter,
  iterSync,
  read,
  readAll,
  readAllInner,
  readAllInnerSized,
  readAllSync,
  readAllSyncSized,
  readSync,
  SeekMode,
  stderr,
  stdin,
  stdout,
  write,
  writeSync,
};
