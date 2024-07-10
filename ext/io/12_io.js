// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Interfaces 100% copied from Go.
// Documentation liberally lifted from them too.
// Thank you! We love Go! <3

import { core, internals, primordials } from "ext:core/mod.js";
import { op_set_raw } from "ext:core/ops";
const {
  Uint8Array,
  ArrayPrototypePush,
  Symbol,
  TypedArrayPrototypeSubarray,
  TypedArrayPrototypeSet,
  TypedArrayPrototypeGetByteLength,
} = primordials;

import {
  readableStreamForRid,
  writableStreamForRid,
} from "ext:deno_web/06_streams.js";

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
  internals.warnOnDeprecatedApi(
    "Deno.copy()",
    new Error().stack,
    "Use `copy()` from `https://jsr.io/@std/io/doc/copy/~` instead.",
  );
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
  internals.warnOnDeprecatedApi(
    "Deno.iter()",
    new Error().stack,
    "Use `ReadableStream` instead.",
  );
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
  internals.warnOnDeprecatedApi(
    "Deno.iterSync()",
    new Error().stack,
    "Use `ReadableStream` instead.",
  );
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

class Stdin {
  #rid = STDIN_RID;
  #ref = true;
  #readable;
  #opPromise;

  constructor() {
  }

  get rid() {
    internals.warnOnDeprecatedApi(
      "Deno.stdin.rid",
      new Error().stack,
      "Use `Deno.stdin` instance methods instead.",
    );
    return this.#rid;
  }

  async read(p) {
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
    core.tryClose(this.#rid);
  }

  get readable() {
    if (this.#readable === undefined) {
      this.#readable = readableStreamForRid(this.#rid);
    }
    return this.#readable;
  }

  setRaw(mode, options = { __proto__: null }) {
    const cbreak = !!(options.cbreak ?? false);
    op_set_raw(this.#rid, mode, cbreak);
  }

  isTerminal() {
    return core.isTerminal(this.#rid);
  }

  [REF]() {
    this.#ref = true;
    if (this.#opPromise) {
      core.refOpPromise(this.#opPromise);
    }
  }

  [UNREF]() {
    this.#ref = false;
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
    internals.warnOnDeprecatedApi(
      "Deno.stdout.rid",
      new Error().stack,
      "Use `Deno.stdout` instance methods instead.",
    );
    return this.#rid;
  }

  write(p) {
    return write(this.#rid, p);
  }

  writeSync(p) {
    return writeSync(this.#rid, p);
  }

  close() {
    core.close(this.#rid);
  }

  get writable() {
    if (this.#writable === undefined) {
      this.#writable = writableStreamForRid(this.#rid);
    }
    return this.#writable;
  }

  isTerminal() {
    return core.isTerminal(this.#rid);
  }
}

class Stderr {
  #rid = STDERR_RID;
  #writable;

  constructor() {
  }

  get rid() {
    internals.warnOnDeprecatedApi(
      "Deno.stderr.rid",
      new Error().stack,
      "Use `Deno.stderr` instance methods instead.",
    );
    return this.#rid;
  }

  write(p) {
    return write(this.#rid, p);
  }

  writeSync(p) {
    return writeSync(this.#rid, p);
  }

  close() {
    core.close(this.#rid);
  }

  get writable() {
    if (this.#writable === undefined) {
      this.#writable = writableStreamForRid(this.#rid);
    }
    return this.#writable;
  }

  isTerminal() {
    return core.isTerminal(this.#rid);
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
