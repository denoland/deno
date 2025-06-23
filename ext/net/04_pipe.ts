// Copyright 2018-2025 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";

const {
  internalRidSymbol,
} = core;
import {
  op_pipe_connect,
  op_pipe_open,
  op_pipe_windows_wait,
} from "ext:core/ops";
const {
  Error,
  ObjectDefineProperty,
  SymbolDispose,
  SafeSet,
  SetPrototypeAdd,
  SetPrototypeDelete,
  SetPrototypeForEach,
  PromisePrototypeThen,
} = primordials;
import {
  readableStreamForRidUnrefable,
  readableStreamForRidUnrefableRef,
  readableStreamForRidUnrefableUnref,
  writableStreamForRid,
} from "ext:deno_web/06_streams.js";

enum PipeMode {
  Message = "message",
  Byte = "byte",
}

type Kind = "unix" | "windows";

interface Options {
  path: string;
  kind: Kind;
}

interface WindowsListenOptions extends Options {
  kind: "windows";
  maxInstances?: number;
  pipeMode: PipeMode;
  inbound?: boolean;
  outbound?: boolean;
}

interface WindowsConnectOptions extends Options {
  kind: "windows";
  read?: boolean;
  write?: boolean;
}

interface UnixListenOptions extends Options {
  kind: "unix";
  mode?: number;
  create?: boolean;
}

async function write(rid, data) {
  return await core.write(rid, data);
}

class Pipe {
  readonly #rid = 0;
  #unref = false;
  #pendingReadPromises = new SafeSet();

  #readable;
  #writable;

  constructor(rid: number) {
    ObjectDefineProperty(this, internalRidSymbol, {
      __proto__: null,
      enumerable: false,
      value: rid,
    });

    this.#rid = rid;
  }

  write(buffer): Promise<number> {
    return write(this.#rid, buffer);
  }

  async read(buffer): Promise<number> {
    if (buffer.length === 0) {
      return 0;
    }
    const promise = core.read(this.#rid, buffer);
    if (this.#unref) core.unrefOpPromise(promise);
    SetPrototypeAdd(this.#pendingReadPromises, promise);
    let nread;
    try {
      nread = await promise;
    } catch (e) {
      throw e;
    } finally {
      SetPrototypeDelete(this.#pendingReadPromises, promise);
    }
    return nread === 0 ? null : nread;
  }

  close() {
    core.close(this.#rid);
  }
  get readable(): ReadableStream<Uint8Array> {
    if (this.#readable === undefined) {
      this.#readable = readableStreamForRidUnrefable(this.#rid);
      if (this.#unref) {
        readableStreamForRidUnrefableUnref(this.#readable);
      }
    }
    return this.#readable;
  }

  get writable(): WritableStream<Uint8Array> {
    if (this.#writable === undefined) {
      this.#writable = writableStreamForRid(this.#rid);
    }
    return this.#writable;
  }

  ref() {
    this.#unref = false;
    if (this.#readable) {
      readableStreamForRidUnrefableRef(this.#readable);
    }

    SetPrototypeForEach(
      this.#pendingReadPromises,
      (promise) => core.refOpPromise(promise),
    );
  }

  unref() {
    this.#unref = true;
    if (this.#readable) {
      readableStreamForRidUnrefableUnref(this.#readable);
    }
    SetPrototypeForEach(
      this.#pendingReadPromises,
      (promise) => core.unrefOpPromise(promise),
    );
  }

  [SymbolDispose]() {
    core.tryClose(this.#rid);
  }
}

async function connect(opts: Options | WindowsConnectOptions) {
  let rid: number;
  switch (opts.kind) {
    case "unix":
      rid = op_pipe_connect(opts.path, "Deno.pipe.connect");
      return new Pipe(rid);
    case "windows":
      rid = await op_pipe_connect(opts, "Deno.pipe.connect");
      return new Pipe(rid);
    default:
      throw new Error(`Unsupported kind: ${opts.kind}`);
  }
}

function open(opts: WindowsListenOptions | UnixListenOptions) {
  let rid: number;
  switch (opts.kind) {
    case "unix":
      rid = op_pipe_open(opts, "Deno.pipe.open");
      return new Pipe(rid);
    case "windows":
      rid = op_pipe_open(opts, "Deno.pipe.open");
      return PromisePrototypeThen(op_pipe_windows_wait(rid), () => new Pipe(rid));
    default:
      throw new Error(`Unsupported kind: ${opts.kind}`);
  }
}

export { connect, open };
