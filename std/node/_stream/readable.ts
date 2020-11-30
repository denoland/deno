// Copyright Node.js contributors. All rights reserved. MIT License.
import {
  ERR_INVALID_OPT_VALUE,
  ERR_METHOD_NOT_IMPLEMENTED,
} from "../_errors.ts";
import type { Encodings } from "../_utils.ts";
import type { Buffer } from "../buffer.ts";
import { captureRejectionSymbol } from "../events.ts";
import { StringDecoder } from "../string_decoder.ts";
import createReadableStreamAsyncIterator from "./async_iterator.ts";
import BufferList from "./buffer_list.ts";
import Duplex, { errorOrDestroy as errorOrDestroyDuplex } from "./duplex.ts";
import streamFrom from "./from.ts";
import {
  _destroy,
  computeNewHighWaterMark,
  emitReadable,
  endReadable,
  errorOrDestroy,
  fromList,
  howMuchToRead,
  nReadingNextTick,
  pipeOnDrain,
  prependListener,
  readableAddChunk,
  resume,
  updateReadableListening,
} from "./readable_internal.ts";
import Stream from "./stream.ts";
import { kDestroy, kPaused } from "./symbols.ts";
import Writable from "./writable.ts";
import { errorOrDestroy as errorOrDestroyWritable } from "./writable_internal.ts";

export interface ReadableOptions {
  autoDestroy?: boolean;
  defaultEncoding?: Encodings;
  destroy?(
    this: Readable,
    error: Error | null,
    callback: (error: Error | null) => void,
  ): void;
  emitClose?: boolean;
  encoding?: Encodings;
  highWaterMark?: number;
  objectMode?: boolean;
  read?(this: Readable): void;
}

export class ReadableState {
  [kPaused]: boolean | null = null;
  awaitDrainWriters: Duplex | Writable | Set<Duplex | Writable> | null = null;
  buffer = new BufferList();
  closed = false;
  closeEmitted = false;
  constructed: boolean;
  decoder: StringDecoder | null = null;
  destroyed = false;
  emittedReadable = false;
  encoding: Encodings | null = null;
  ended = false;
  endEmitted = false;
  errored: Error | null = null;
  errorEmitted = false;
  flowing: boolean | null = null;
  highWaterMark: number;
  length = 0;
  multiAwaitDrain = false;
  needReadable = false;
  objectMode: boolean;
  pipes: Array<Duplex | Writable> = [];
  readable = true;
  readableListening = false;
  reading = false;
  readingMore = false;
  resumeScheduled = false;
  sync = true;
  emitClose: boolean;
  autoDestroy: boolean;
  defaultEncoding: string;

  constructor(options?: ReadableOptions) {
    this.objectMode = !!options?.objectMode;

    this.highWaterMark = options?.highWaterMark ??
      (this.objectMode ? 16 : 16 * 1024);
    if (Number.isInteger(this.highWaterMark) && this.highWaterMark >= 0) {
      this.highWaterMark = Math.floor(this.highWaterMark);
    } else {
      throw new ERR_INVALID_OPT_VALUE("highWaterMark", this.highWaterMark);
    }

    this.emitClose = options?.emitClose ?? true;
    this.autoDestroy = options?.autoDestroy ?? true;
    this.defaultEncoding = options?.defaultEncoding || "utf8";

    if (options?.encoding) {
      this.decoder = new StringDecoder(options.encoding);
      this.encoding = options.encoding;
    }

    this.constructed = true;
  }
}

class Readable extends Stream {
  _readableState: ReadableState;

  constructor(options?: ReadableOptions) {
    super();
    if (options) {
      if (typeof options.read === "function") {
        this._read = options.read;
      }
      if (typeof options.destroy === "function") {
        this._destroy = options.destroy;
      }
    }
    this._readableState = new ReadableState(options);
  }

  static from(
    // deno-lint-ignore no-explicit-any
    iterable: Iterable<any> | AsyncIterable<any>,
    opts?: ReadableOptions,
  ): Readable {
    return streamFrom(iterable, opts);
  }

  static ReadableState = ReadableState;

  static _fromList = fromList;

  // You can override either this method, or the async _read(n) below.
  read(n?: number) {
    // Same as parseInt(undefined, 10), however V8 7.3 performance regressed
    // in this scenario, so we are doing it manually.
    if (n === undefined) {
      n = NaN;
    }
    const state = this._readableState;
    const nOrig = n;

    if (n > state.highWaterMark) {
      state.highWaterMark = computeNewHighWaterMark(n);
    }

    if (n !== 0) {
      state.emittedReadable = false;
    }

    if (
      n === 0 &&
      state.needReadable &&
      ((state.highWaterMark !== 0
        ? state.length >= state.highWaterMark
        : state.length > 0) ||
        state.ended)
    ) {
      if (state.length === 0 && state.ended) {
        endReadable(this);
      } else {
        emitReadable(this);
      }
      return null;
    }

    n = howMuchToRead(n, state);

    if (n === 0 && state.ended) {
      if (state.length === 0) {
        endReadable(this);
      }
      return null;
    }

    let doRead = state.needReadable;
    if (
      state.length === 0 || state.length - (n as number) < state.highWaterMark
    ) {
      doRead = true;
    }

    if (
      state.ended || state.reading || state.destroyed || state.errored ||
      !state.constructed
    ) {
      doRead = false;
    } else if (doRead) {
      state.reading = true;
      state.sync = true;
      if (state.length === 0) {
        state.needReadable = true;
      }
      this._read();
      state.sync = false;
      if (!state.reading) {
        n = howMuchToRead(nOrig, state);
      }
    }

    let ret;
    if ((n as number) > 0) {
      ret = fromList((n as number), state);
    } else {
      ret = null;
    }

    if (ret === null) {
      state.needReadable = state.length <= state.highWaterMark;
      n = 0;
    } else {
      state.length -= n as number;
      if (state.multiAwaitDrain) {
        (state.awaitDrainWriters as Set<Writable>).clear();
      } else {
        state.awaitDrainWriters = null;
      }
    }

    if (state.length === 0) {
      if (!state.ended) {
        state.needReadable = true;
      }

      if (nOrig !== n && state.ended) {
        endReadable(this);
      }
    }

    if (ret !== null) {
      this.emit("data", ret);
    }

    return ret;
  }

  _read(_size?: number) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("_read()");
  }

  pipe<T extends Duplex | Writable>(dest: T, pipeOpts?: { end?: boolean }): T {
    // deno-lint-ignore no-this-alias
    const src = this;
    const state = this._readableState;

    if (state.pipes.length === 1) {
      if (!state.multiAwaitDrain) {
        state.multiAwaitDrain = true;
        state.awaitDrainWriters = new Set(
          state.awaitDrainWriters ? [state.awaitDrainWriters as Writable] : [],
        );
      }
    }

    state.pipes.push(dest);

    const doEnd = (!pipeOpts || pipeOpts.end !== false);

    //TODO(Soremwar)
    //Part of doEnd condition
    //In  node, output/inout are a duplex Stream
    // &&
    // dest !== stdout &&
    // dest !== stderr

    const endFn = doEnd ? onend : unpipe;
    if (state.endEmitted) {
      queueMicrotask(endFn);
    } else {
      this.once("end", endFn);
    }

    dest.on("unpipe", onunpipe);
    function onunpipe(readable: Readable, unpipeInfo: { hasUnpiped: boolean }) {
      if (readable === src) {
        if (unpipeInfo && unpipeInfo.hasUnpiped === false) {
          unpipeInfo.hasUnpiped = true;
          cleanup();
        }
      }
    }

    function onend() {
      dest.end();
    }

    let ondrain: () => void;

    let cleanedUp = false;
    function cleanup() {
      dest.removeListener("close", onclose);
      dest.removeListener("finish", onfinish);
      if (ondrain) {
        dest.removeListener("drain", ondrain);
      }
      dest.removeListener("error", onerror);
      dest.removeListener("unpipe", onunpipe);
      src.removeListener("end", onend);
      src.removeListener("end", unpipe);
      src.removeListener("data", ondata);

      cleanedUp = true;
      if (
        ondrain && state.awaitDrainWriters &&
        (!dest._writableState || dest._writableState.needDrain)
      ) {
        ondrain();
      }
    }

    this.on("data", ondata);
    // deno-lint-ignore no-explicit-any
    function ondata(chunk: any) {
      const ret = dest.write(chunk);
      if (ret === false) {
        if (!cleanedUp) {
          if (state.pipes.length === 1 && state.pipes[0] === dest) {
            state.awaitDrainWriters = dest;
            state.multiAwaitDrain = false;
          } else if (state.pipes.length > 1 && state.pipes.includes(dest)) {
            (state.awaitDrainWriters as Set<Duplex | Writable>).add(dest);
          }
          src.pause();
        }
        if (!ondrain) {
          ondrain = pipeOnDrain(src, dest);
          dest.on("drain", ondrain);
        }
      }
    }

    function onerror(er: Error) {
      unpipe();
      dest.removeListener("error", onerror);
      if (dest.listenerCount("error") === 0) {
        const s = dest._writableState || (dest as Duplex)._readableState;
        if (s && !s.errorEmitted) {
          if (dest instanceof Duplex) {
            errorOrDestroyDuplex(dest as unknown as Duplex, er);
          } else {
            errorOrDestroyWritable(dest as Writable, er);
          }
        } else {
          dest.emit("error", er);
        }
      }
    }

    prependListener(dest, "error", onerror);

    function onclose() {
      dest.removeListener("finish", onfinish);
      unpipe();
    }
    dest.once("close", onclose);
    function onfinish() {
      dest.removeListener("close", onclose);
      unpipe();
    }
    dest.once("finish", onfinish);

    function unpipe() {
      src.unpipe(dest as Writable);
    }

    dest.emit("pipe", this);

    if (!state.flowing) {
      this.resume();
    }

    return dest;
  }

  isPaused() {
    return this._readableState[kPaused] === true ||
      this._readableState.flowing === false;
  }

  setEncoding(enc: Encodings) {
    const decoder = new StringDecoder(enc);
    this._readableState.decoder = decoder;
    this._readableState.encoding = this._readableState.decoder
      .encoding as Encodings;

    const buffer = this._readableState.buffer;
    let content = "";
    for (const data of buffer) {
      content += decoder.write(data as Buffer);
    }
    buffer.clear();
    if (content !== "") {
      buffer.push(content);
    }
    this._readableState.length = content.length;
    return this;
  }

  on(
    event: "close" | "end" | "pause" | "readable" | "resume",
    listener: () => void,
  ): this;
  // deno-lint-ignore no-explicit-any
  on(event: "data", listener: (chunk: any) => void): this;
  on(event: "error", listener: (err: Error) => void): this;
  // deno-lint-ignore no-explicit-any
  on(event: string | symbol, listener: (...args: any[]) => void): this;
  on(
    ev: string | symbol,
    fn:
      | (() => void)
      // deno-lint-ignore no-explicit-any
      | ((chunk: any) => void)
      | ((err: Error) => void)
      // deno-lint-ignore no-explicit-any
      | ((...args: any[]) => void),
  ) {
    const res = super.on.call(this, ev, fn);
    const state = this._readableState;

    if (ev === "data") {
      state.readableListening = this.listenerCount("readable") > 0;

      if (state.flowing !== false) {
        this.resume();
      }
    } else if (ev === "readable") {
      if (!state.endEmitted && !state.readableListening) {
        state.readableListening = state.needReadable = true;
        state.flowing = false;
        state.emittedReadable = false;
        if (state.length) {
          emitReadable(this);
        } else if (!state.reading) {
          queueMicrotask(() => nReadingNextTick(this));
        }
      }
    }

    return res;
  }

  removeListener(
    event: "close" | "end" | "pause" | "readable" | "resume",
    listener: () => void,
  ): this;
  // deno-lint-ignore no-explicit-any
  removeListener(event: "data", listener: (chunk: any) => void): this;
  removeListener(event: "error", listener: (err: Error) => void): this;
  removeListener(
    event: string | symbol,
    // deno-lint-ignore no-explicit-any
    listener: (...args: any[]) => void,
  ): this;
  removeListener(
    ev: string | symbol,
    fn:
      | (() => void)
      // deno-lint-ignore no-explicit-any
      | ((chunk: any) => void)
      | ((err: Error) => void)
      // deno-lint-ignore no-explicit-any
      | ((...args: any[]) => void),
  ) {
    const res = super.removeListener.call(this, ev, fn);

    if (ev === "readable") {
      queueMicrotask(() => updateReadableListening(this));
    }

    return res;
  }

  off = this.removeListener;

  destroy(err?: Error | null, cb?: () => void) {
    const r = this._readableState;

    if (r.destroyed) {
      if (typeof cb === "function") {
        cb();
      }

      return this;
    }

    if (err) {
      // Avoid V8 leak, https://github.com/nodejs/node/pull/34103#issuecomment-652002364
      err.stack;

      if (!r.errored) {
        r.errored = err;
      }
    }

    r.destroyed = true;

    // If still constructing then defer calling _destroy.
    if (!r.constructed) {
      this.once(kDestroy, (er: Error) => {
        _destroy(this, err || er, cb);
      });
    } else {
      _destroy(this, err, cb);
    }

    return this;
  }

  _undestroy() {
    const r = this._readableState;
    r.constructed = true;
    r.closed = false;
    r.closeEmitted = false;
    r.destroyed = false;
    r.errored = null;
    r.errorEmitted = false;
    r.reading = false;
    r.ended = false;
    r.endEmitted = false;
  }

  _destroy(
    error: Error | null,
    callback: (error?: Error | null) => void,
  ): void {
    callback(error);
  }

  [captureRejectionSymbol](err: Error) {
    this.destroy(err);
  }

  // deno-lint-ignore no-explicit-any
  push(chunk: any, encoding?: Encodings): boolean {
    return readableAddChunk(this, chunk, encoding, false);
  }

  // deno-lint-ignore no-explicit-any
  unshift(chunk: any, encoding?: string): boolean {
    return readableAddChunk(this, chunk, encoding, true);
  }

  unpipe(dest?: Writable): this {
    const state = this._readableState;
    const unpipeInfo = { hasUnpiped: false };

    if (state.pipes.length === 0) {
      return this;
    }

    if (!dest) {
      // remove all.
      const dests = state.pipes;
      state.pipes = [];
      this.pause();

      for (const dest of dests) {
        dest.emit("unpipe", this, { hasUnpiped: false });
      }
      return this;
    }

    const index = state.pipes.indexOf(dest);
    if (index === -1) {
      return this;
    }

    state.pipes.splice(index, 1);
    if (state.pipes.length === 0) {
      this.pause();
    }

    dest.emit("unpipe", this, unpipeInfo);

    return this;
  }

  removeAllListeners(
    ev:
      | "close"
      | "data"
      | "end"
      | "error"
      | "pause"
      | "readable"
      | "resume"
      | symbol
      | undefined,
  ) {
    const res = super.removeAllListeners(ev);

    if (ev === "readable" || ev === undefined) {
      queueMicrotask(() => updateReadableListening(this));
    }

    return res;
  }

  resume() {
    const state = this._readableState;
    if (!state.flowing) {
      // We flow only if there is no one listening
      // for readable, but we still have to call
      // resume().
      state.flowing = !state.readableListening;
      resume(this, state);
    }
    state[kPaused] = false;
    return this;
  }

  pause() {
    if (this._readableState.flowing !== false) {
      this._readableState.flowing = false;
      this.emit("pause");
    }
    this._readableState[kPaused] = true;
    return this;
  }

  /** Wrap an old-style stream as the async data source. */
  wrap(stream: Stream): this {
    const state = this._readableState;
    let paused = false;

    stream.on("end", () => {
      if (state.decoder && !state.ended) {
        const chunk = state.decoder.end();
        if (chunk && chunk.length) {
          this.push(chunk);
        }
      }

      this.push(null);
    });

    stream.on("data", (chunk) => {
      if (state.decoder) {
        chunk = state.decoder.write(chunk);
      }

      if (state.objectMode && (chunk === null || chunk === undefined)) {
        return;
      } else if (!state.objectMode && (!chunk || !chunk.length)) {
        return;
      }

      const ret = this.push(chunk);
      if (!ret) {
        paused = true;
        // By the time this is triggered, stream will be a readable stream
        // deno-lint-ignore ban-ts-comment
        // @ts-ignore
        stream.pause();
      }
    });

    // TODO(Soremwar)
    // There must be a clean way to implement this on TypeScript
    // Proxy all the other methods. Important when wrapping filters and duplexes.
    for (const i in stream) {
      // deno-lint-ignore ban-ts-comment
      //@ts-ignore
      if (this[i] === undefined && typeof stream[i] === "function") {
        // deno-lint-ignore ban-ts-comment
        //@ts-ignore
        this[i] = function methodWrap(method) {
          return function methodWrapReturnFunction() {
            // deno-lint-ignore ban-ts-comment
            //@ts-ignore
            return stream[method].apply(stream);
          };
        }(i);
      }
    }

    stream.on("error", (err) => {
      errorOrDestroy(this, err);
    });

    stream.on("close", () => {
      this.emit("close");
    });

    stream.on("destroy", () => {
      this.emit("destroy");
    });

    stream.on("pause", () => {
      this.emit("pause");
    });

    stream.on("resume", () => {
      this.emit("resume");
    });

    this._read = () => {
      if (paused) {
        paused = false;
        // By the time this is triggered, stream will be a readable stream
        // deno-lint-ignore ban-ts-comment
        //@ts-ignore
        stream.resume();
      }
    };

    return this;
  }

  [Symbol.asyncIterator]() {
    return createReadableStreamAsyncIterator(this);
  }

  get readable(): boolean {
    return this._readableState?.readable &&
      !this._readableState?.destroyed &&
      !this._readableState?.errorEmitted &&
      !this._readableState?.endEmitted;
  }
  set readable(val: boolean) {
    if (this._readableState) {
      this._readableState.readable = val;
    }
  }

  get readableHighWaterMark(): number {
    return this._readableState.highWaterMark;
  }

  get readableBuffer() {
    return this._readableState && this._readableState.buffer;
  }

  get readableFlowing(): boolean | null {
    return this._readableState.flowing;
  }

  set readableFlowing(state: boolean | null) {
    if (this._readableState) {
      this._readableState.flowing = state;
    }
  }

  get readableLength() {
    return this._readableState.length;
  }

  get readableObjectMode() {
    return this._readableState ? this._readableState.objectMode : false;
  }

  get readableEncoding() {
    return this._readableState ? this._readableState.encoding : null;
  }

  get destroyed() {
    if (this._readableState === undefined) {
      return false;
    }
    return this._readableState.destroyed;
  }

  set destroyed(value: boolean) {
    if (!this._readableState) {
      return;
    }
    this._readableState.destroyed = value;
  }

  get readableEnded() {
    return this._readableState ? this._readableState.endEmitted : false;
  }
}

Object.defineProperties(Readable, {
  _readableState: { enumerable: false },
  destroyed: { enumerable: false },
  readableBuffer: { enumerable: false },
  readableEncoding: { enumerable: false },
  readableEnded: { enumerable: false },
  readableFlowing: { enumerable: false },
  readableHighWaterMark: { enumerable: false },
  readableLength: { enumerable: false },
  readableObjectMode: { enumerable: false },
});

export default Readable;
