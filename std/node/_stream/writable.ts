// Copyright Node.js contributors. All rights reserved. MIT License.
import {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_OPT_VALUE,
  ERR_METHOD_NOT_IMPLEMENTED,
  ERR_STREAM_ALREADY_FINISHED,
  ERR_STREAM_CANNOT_PIPE,
  ERR_STREAM_DESTROYED,
  ERR_STREAM_NULL_VALUES,
  ERR_STREAM_WRITE_AFTER_END,
  ERR_UNKNOWN_ENCODING,
} from "../_errors.ts";
import type { Encodings } from "../_utils.ts";
import { Buffer } from "../buffer.ts";
import { captureRejectionSymbol } from "../events.ts";
import Stream from "./stream.ts";
import type { AfterWriteTick, writeV } from "./writable_internal.ts";
import {
  clearBuffer,
  destroy,
  errorBuffer,
  errorOrDestroy,
  finishMaybe,
  kOnFinished,
  nop,
  onwrite,
  resetBuffer,
  writeOrBuffer,
} from "./writable_internal.ts";

type WritableEncodings = Encodings | "buffer";

export interface WritableOptions {
  autoDestroy?: boolean;
  decodeStrings?: boolean;
  defaultEncoding?: WritableEncodings;
  destroy?(
    this: Writable,
    error: Error | null,
    callback: (error: Error | null) => void,
  ): void;
  emitClose?: boolean;
  final?(this: Writable, callback: (error?: Error | null) => void): void;
  highWaterMark?: number;
  objectMode?: boolean;
  write?(
    this: Writable,
    // deno-lint-ignore no-explicit-any
    chunk: any,
    encoding: WritableEncodings,
    callback: (error?: Error | null) => void,
  ): void;
  writev?(
    this: Writable,
    // deno-lint-ignore no-explicit-any
    chunks: Array<{ chunk: any; encoding: string }>,
    callback: (error?: Error | null) => void,
  ): void;
}

export class WritableState {
  [kOnFinished]: Array<(error?: Error) => void> = [];
  afterWriteTickInfo: null | AfterWriteTick = null;
  allBuffers = true;
  allNoop = true;
  autoDestroy: boolean;
  buffered: Array<{
    allBuffers?: boolean;
    // deno-lint-ignore no-explicit-any
    chunk: any;
    encoding: string;
    callback: (error: Error) => void;
  }> = [];
  bufferedIndex = 0;
  bufferProcessing = false;
  closed = false;
  closeEmitted = false;
  constructed: boolean;
  corked = 0;
  decodeStrings: boolean;
  defaultEncoding: WritableEncodings;
  destroyed = false;
  emitClose: boolean;
  ended = false;
  ending = false;
  errored: Error | null = null;
  errorEmitted = false;
  finalCalled = false;
  finished = false;
  highWaterMark: number;
  length = 0;
  needDrain = false;
  objectMode: boolean;
  onwrite: (error?: Error | null) => void;
  pendingcb = 0;
  prefinished = false;
  sync = true;
  writecb: null | ((error: Error) => void) = null;
  writable = true;
  writelen = 0;
  writing = false;

  constructor(options: WritableOptions | undefined, stream: Writable) {
    this.objectMode = !!options?.objectMode;

    this.highWaterMark = options?.highWaterMark ??
      (this.objectMode ? 16 : 16 * 1024);

    if (Number.isInteger(this.highWaterMark) && this.highWaterMark >= 0) {
      this.highWaterMark = Math.floor(this.highWaterMark);
    } else {
      throw new ERR_INVALID_OPT_VALUE("highWaterMark", this.highWaterMark);
    }

    this.decodeStrings = !options?.decodeStrings === false;

    this.defaultEncoding = options?.defaultEncoding || "utf8";

    this.onwrite = onwrite.bind(undefined, stream);

    resetBuffer(this);

    this.emitClose = options?.emitClose ?? true;
    this.autoDestroy = options?.autoDestroy ?? true;
    this.constructed = true;
  }

  getBuffer() {
    return this.buffered.slice(this.bufferedIndex);
  }

  get bufferedRequestCount() {
    return this.buffered.length - this.bufferedIndex;
  }
}

/** A bit simpler than readable streams.
* Implement an async `._write(chunk, encoding, cb)`, and it'll handle all
* the drain event emission and buffering.
*/
class Writable extends Stream {
  _final?: (
    this: Writable,
    callback: (error?: Error | null | undefined) => void,
  ) => void;
  _writableState: WritableState;
  _writev?: writeV | null = null;

  constructor(options?: WritableOptions) {
    super();
    this._writableState = new WritableState(options, this);

    if (options) {
      if (typeof options.write === "function") {
        this._write = options.write;
      }

      if (typeof options.writev === "function") {
        this._writev = options.writev;
      }

      if (typeof options.destroy === "function") {
        this._destroy = options.destroy;
      }

      if (typeof options.final === "function") {
        this._final = options.final;
      }
    }
  }

  [captureRejectionSymbol](err?: Error) {
    this.destroy(err);
  }

  static WritableState = WritableState;

  get destroyed() {
    return this._writableState ? this._writableState.destroyed : false;
  }

  set destroyed(value) {
    if (this._writableState) {
      this._writableState.destroyed = value;
    }
  }

  get writable() {
    const w = this._writableState;
    return !w.destroyed && !w.errored && !w.ending && !w.ended;
  }

  set writable(val) {
    if (this._writableState) {
      this._writableState.writable = !!val;
    }
  }

  get writableFinished() {
    return this._writableState ? this._writableState.finished : false;
  }

  get writableObjectMode() {
    return this._writableState ? this._writableState.objectMode : false;
  }

  get writableBuffer() {
    return this._writableState && this._writableState.getBuffer();
  }

  get writableEnded() {
    return this._writableState ? this._writableState.ending : false;
  }

  get writableHighWaterMark() {
    return this._writableState && this._writableState.highWaterMark;
  }

  get writableCorked() {
    return this._writableState ? this._writableState.corked : 0;
  }

  get writableLength() {
    return this._writableState && this._writableState.length;
  }

  _undestroy() {
    const w = this._writableState;
    w.constructed = true;
    w.destroyed = false;
    w.closed = false;
    w.closeEmitted = false;
    w.errored = null;
    w.errorEmitted = false;
    w.ended = false;
    w.ending = false;
    w.finalCalled = false;
    w.prefinished = false;
    w.finished = false;
  }

  _destroy(err: Error | null, cb: (error?: Error | null) => void) {
    cb(err);
  }

  destroy(err?: Error | null, cb?: () => void) {
    const state = this._writableState;
    if (!state.destroyed) {
      queueMicrotask(() => errorBuffer(state));
    }
    destroy.call(this, err, cb);
    return this;
  }

  end(cb?: () => void): void;
  // deno-lint-ignore no-explicit-any
  end(chunk: any, cb?: () => void): void;
  // deno-lint-ignore no-explicit-any
  end(chunk: any, encoding: WritableEncodings, cb?: () => void): void;

  end(
    // deno-lint-ignore no-explicit-any
    x?: any | (() => void),
    y?: WritableEncodings | (() => void),
    z?: () => void,
  ) {
    const state = this._writableState;
    // deno-lint-ignore no-explicit-any
    let chunk: any | null;
    let encoding: WritableEncodings | null;
    let cb: undefined | ((error?: Error) => void);

    if (typeof x === "function") {
      chunk = null;
      encoding = null;
      cb = x;
    } else if (typeof y === "function") {
      chunk = x;
      encoding = null;
      cb = y;
    } else {
      chunk = x;
      encoding = y as WritableEncodings;
      cb = z;
    }

    if (chunk !== null && chunk !== undefined) {
      this.write(chunk, encoding);
    }

    if (state.corked) {
      state.corked = 1;
      this.uncork();
    }

    let err: Error | undefined;
    if (!state.errored && !state.ending) {
      state.ending = true;
      finishMaybe(this, state, true);
      state.ended = true;
    } else if (state.finished) {
      err = new ERR_STREAM_ALREADY_FINISHED("end");
    } else if (state.destroyed) {
      err = new ERR_STREAM_DESTROYED("end");
    }

    if (typeof cb === "function") {
      if (err || state.finished) {
        queueMicrotask(() => {
          (cb as (error?: Error | undefined) => void)(err);
        });
      } else {
        state[kOnFinished].push(cb);
      }
    }

    return this;
  }

  _write(
    // deno-lint-ignore no-explicit-any
    chunk: any,
    encoding: string,
    cb: (error?: Error | null) => void,
  ): void {
    if (this._writev) {
      this._writev([{ chunk, encoding }], cb);
    } else {
      throw new ERR_METHOD_NOT_IMPLEMENTED("_write()");
    }
  }

  //This signature was changed to keep inheritance coherent
  pipe(dest: Writable): Writable {
    errorOrDestroy(this, new ERR_STREAM_CANNOT_PIPE());
    return dest;
  }

  // deno-lint-ignore no-explicit-any
  write(chunk: any, cb?: (error: Error | null | undefined) => void): boolean;
  write(
    // deno-lint-ignore no-explicit-any
    chunk: any,
    encoding: WritableEncodings | null,
    cb?: (error: Error | null | undefined) => void,
  ): boolean;

  write(
    // deno-lint-ignore no-explicit-any
    chunk: any,
    x?: WritableEncodings | null | ((error: Error | null | undefined) => void),
    y?: ((error: Error | null | undefined) => void),
  ) {
    const state = this._writableState;
    let encoding: WritableEncodings;
    let cb: (error?: Error | null) => void;

    if (typeof x === "function") {
      cb = x;
      encoding = state.defaultEncoding;
    } else {
      if (!x) {
        encoding = state.defaultEncoding;
      } else if (x !== "buffer" && !Buffer.isEncoding(x)) {
        throw new ERR_UNKNOWN_ENCODING(x);
      } else {
        encoding = x;
      }
      if (typeof y !== "function") {
        cb = nop;
      } else {
        cb = y;
      }
    }

    if (chunk === null) {
      throw new ERR_STREAM_NULL_VALUES();
    } else if (!state.objectMode) {
      if (typeof chunk === "string") {
        if (state.decodeStrings !== false) {
          chunk = Buffer.from(chunk, encoding);
          encoding = "buffer";
        }
      } else if (chunk instanceof Buffer) {
        encoding = "buffer";
      } else if (Stream._isUint8Array(chunk)) {
        chunk = Stream._uint8ArrayToBuffer(chunk);
        encoding = "buffer";
      } else {
        throw new ERR_INVALID_ARG_TYPE(
          "chunk",
          ["string", "Buffer", "Uint8Array"],
          chunk,
        );
      }
    }

    let err: Error | undefined;
    if (state.ending) {
      err = new ERR_STREAM_WRITE_AFTER_END();
    } else if (state.destroyed) {
      err = new ERR_STREAM_DESTROYED("write");
    }

    if (err) {
      queueMicrotask(() => cb(err));
      errorOrDestroy(this, err, true);
      return false;
    }
    state.pendingcb++;
    return writeOrBuffer(this, state, chunk, encoding, cb);
  }

  cork() {
    this._writableState.corked++;
  }

  uncork() {
    const state = this._writableState;

    if (state.corked) {
      state.corked--;

      if (!state.writing) {
        clearBuffer(this, state);
      }
    }
  }

  setDefaultEncoding(encoding: string) {
    // node::ParseEncoding() requires lower case.
    if (typeof encoding === "string") {
      encoding = encoding.toLowerCase();
    }
    if (!Buffer.isEncoding(encoding)) {
      throw new ERR_UNKNOWN_ENCODING(encoding);
    }
    this._writableState.defaultEncoding = encoding as WritableEncodings;
    return this;
  }
}

export default Writable;
