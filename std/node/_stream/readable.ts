// Copyright Node.js contributors. All rights reserved. MIT License.
import EventEmitter, { captureRejectionSymbol } from "../events.ts";
import Stream from "./stream.ts";
import { Buffer } from "../buffer.ts";
import BufferList from "./buffer_list.ts";
import {
  ERR_INVALID_OPT_VALUE,
  ERR_METHOD_NOT_IMPLEMENTED,
  ERR_MULTIPLE_CALLBACK,
  ERR_STREAM_PUSH_AFTER_EOF,
  ERR_STREAM_UNSHIFT_AFTER_END_EVENT,
} from "../_errors.ts";
import { StringDecoder } from "../string_decoder.ts";
import createReadableStreamAsyncIterator from "./async_iterator.ts";
import streamFrom from "./from.ts";
import { kConstruct, kDestroy, kPaused } from "./symbols.ts";
import type Writable from "./writable.ts";
import { errorOrDestroy as errorOrDestroyDuplex } from "./duplex.ts";

function construct(stream: Readable, cb: () => void) {
  const r = stream._readableState;

  if (!stream._construct) {
    return;
  }

  stream.once(kConstruct, cb);

  r.constructed = false;

  queueMicrotask(() => {
    let called = false;
    stream._construct?.((err?: Error) => {
      r.constructed = true;

      if (called) {
        err = new ERR_MULTIPLE_CALLBACK();
      } else {
        called = true;
      }

      if (r.destroyed) {
        stream.emit(kDestroy, err);
      } else if (err) {
        errorOrDestroy(stream, err, true);
      } else {
        queueMicrotask(() => stream.emit(kConstruct));
      }
    });
  });
}

function _destroy(
  self: Readable,
  err?: Error,
  cb?: (error?: Error | null) => void,
) {
  self._destroy(err || null, (err) => {
    const r = (self as Readable)._readableState;

    if (err) {
      // Avoid V8 leak, https://github.com/nodejs/node/pull/34103#issuecomment-652002364
      err.stack;

      if (!r.errored) {
        r.errored = err;
      }
    }

    r.closed = true;

    if (typeof cb === "function") {
      cb(err);
    }

    if (err) {
      queueMicrotask(() => {
        if (!r.errorEmitted) {
          r.errorEmitted = true;
          self.emit("error", err);
        }
        r.closeEmitted = true;
        if (r.emitClose) {
          self.emit("close");
        }
      });
    } else {
      queueMicrotask(() => {
        r.closeEmitted = true;
        if (r.emitClose) {
          self.emit("close");
        }
      });
    }
  });
}

function errorOrDestroy(stream: Readable, err: Error, sync = false) {
  const r = stream._readableState;

  if (r.destroyed) {
    return stream;
  }

  if (r.autoDestroy) {
    stream.destroy(err);
  } else if (err) {
    // Avoid V8 leak, https://github.com/nodejs/node/pull/34103#issuecomment-652002364
    err.stack;

    if (!r.errored) {
      r.errored = err;
    }
    if (sync) {
      queueMicrotask(() => {
        if (!r.errorEmitted) {
          r.errorEmitted = true;
          stream.emit("error", err);
        }
      });
    } else if (!r.errorEmitted) {
      r.errorEmitted = true;
      stream.emit("error", err);
    }
  }
}

function flow(stream: Readable) {
  const state = stream._readableState;
  while (state.flowing && stream.read() !== null);
}

function pipeOnDrain(src: Readable, dest: Writable) {
  return function pipeOnDrainFunctionResult() {
    const state = src._readableState;

    if (state.awaitDrainWriters === dest) {
      state.awaitDrainWriters = null;
    } else if (state.multiAwaitDrain) {
      (state.awaitDrainWriters as Set<Writable>).delete(dest);
    }

    if (
      (!state.awaitDrainWriters ||
        (state.awaitDrainWriters as Set<Writable>).size === 0) &&
      src.listenerCount("data")
    ) {
      state.flowing = true;
      flow(src);
    }
  };
}

function updateReadableListening(self: Readable) {
  const state = self._readableState;
  state.readableListening = self.listenerCount("readable") > 0;

  if (state.resumeScheduled && state[kPaused] === false) {
    // Flowing needs to be set to true now, otherwise
    // the upcoming resume will not flow.
    state.flowing = true;

    // Crude way to check if we should resume.
  } else if (self.listenerCount("data") > 0) {
    self.resume();
  } else if (!state.readableListening) {
    state.flowing = null;
  }
}

function nReadingNextTick(self: Readable) {
  self.read(0);
}

function resume(stream: Readable, state: ReadableState) {
  if (!state.resumeScheduled) {
    state.resumeScheduled = true;
    queueMicrotask(() => resume_(stream, state));
  }
}

function resume_(stream: Readable, state: ReadableState) {
  if (!state.reading) {
    stream.read(0);
  }

  state.resumeScheduled = false;
  stream.emit("resume");
  flow(stream);
  if (state.flowing && !state.reading) {
    stream.read(0);
  }
}

function readableAddChunk(
  stream: Readable,
  chunk: string | Buffer | Uint8Array | null,
  encoding: undefined | string = undefined,
  addToFront: boolean,
) {
  const state = stream._readableState;
  let usedEncoding = encoding;

  let err;
  if (!state.objectMode) {
    if (typeof chunk === "string") {
      usedEncoding = encoding || state.defaultEncoding;
      if (state.encoding !== usedEncoding) {
        if (addToFront && state.encoding) {
          chunk = Buffer.from(chunk, usedEncoding).toString(state.encoding);
        } else {
          chunk = Buffer.from(chunk, usedEncoding);
          usedEncoding = "";
        }
      }
    } else if (chunk instanceof Uint8Array) {
      chunk = Buffer.from(chunk);
    }
  }

  if (err) {
    errorOrDestroy(stream, err);
  } else if (chunk === null) {
    state.reading = false;
    onEofChunk(stream, state);
  } else if (state.objectMode || (chunk.length > 0)) {
    if (addToFront) {
      if (state.endEmitted) {
        errorOrDestroy(stream, new ERR_STREAM_UNSHIFT_AFTER_END_EVENT());
      } else {
        addChunk(stream, state, chunk, true);
      }
    } else if (state.ended) {
      errorOrDestroy(stream, new ERR_STREAM_PUSH_AFTER_EOF());
    } else if (state.destroyed || state.errored) {
      return false;
    } else {
      state.reading = false;
      if (state.decoder && !usedEncoding) {
        //TODO(Soremwar)
        //I don't think this cast is right
        chunk = state.decoder.write(Buffer.from(chunk as Uint8Array));
        if (state.objectMode || chunk.length !== 0) {
          addChunk(stream, state, chunk, false);
        } else {
          maybeReadMore(stream, state);
        }
      } else {
        addChunk(stream, state, chunk, false);
      }
    }
  } else if (!addToFront) {
    state.reading = false;
    maybeReadMore(stream, state);
  }

  return !state.ended &&
    (state.length < state.highWaterMark || state.length === 0);
}

function addChunk(
  stream: Readable,
  state: ReadableState,
  chunk: string | Buffer | Uint8Array,
  addToFront: boolean,
) {
  if (state.flowing && state.length === 0 && !state.sync) {
    if (state.multiAwaitDrain) {
      (state.awaitDrainWriters as Set<Writable>).clear();
    } else {
      state.awaitDrainWriters = null;
    }
    stream.emit("data", chunk);
  } else {
    // Update the buffer info.
    state.length += state.objectMode ? 1 : chunk.length;
    if (addToFront) {
      state.buffer.unshift(chunk);
    } else {
      state.buffer.push(chunk);
    }

    if (state.needReadable) {
      emitReadable(stream);
    }
  }
  maybeReadMore(stream, state);
}

function prependListener(
  emitter: EventEmitter,
  event: string,
  // deno-lint-ignore no-explicit-any
  fn: (...args: any[]) => any,
) {
  if (typeof emitter.prependListener === "function") {
    return emitter.prependListener(event, fn);
  }

  // This is a hack to make sure that our error handler is attached before any
  // userland ones.  NEVER DO THIS. This is here only because this code needs
  // to continue to work with older versions of Node.js that do not include
  //the prependListener() method. The goal is to eventually remove this hack.
  // TODO(Soremwar)
  // Burn it with fire
  // deno-lint-ignore ban-ts-comment
  //@ts-ignore
  if (emitter._events.get(event)?.length) {
    // deno-lint-ignore ban-ts-comment
    //@ts-ignore
    const listeners = [fn, ...emitter._events.get(event)];
    // deno-lint-ignore ban-ts-comment
    //@ts-ignore
    emitter._events.set(event, listeners);
  } else {
    emitter.on(event, fn);
  }
}

/** Pluck off n bytes from an array of buffers.
* Length is the combined lengths of all the buffers in the list.
* This function is designed to be inlinable, so please take care when making
* changes to the function body.
*/
function fromList(n: number, state: ReadableState) {
  // nothing buffered.
  if (state.length === 0) {
    return null;
  }

  let ret;
  if (state.objectMode) {
    ret = state.buffer.shift();
  } else if (!n || n >= state.length) {
    if (state.decoder) {
      ret = state.buffer.join("");
    } else if (state.buffer.length === 1) {
      ret = state.buffer.first();
    } else {
      ret = state.buffer.concat(state.length);
    }
    state.buffer.clear();
  } else {
    ret = state.buffer.consume(n, !!state.decoder);
  }

  return ret;
}

function endReadable(stream: Readable) {
  const state = stream._readableState;

  if (!state.endEmitted) {
    state.ended = true;
    queueMicrotask(() => endReadableNT(state, stream));
  }
}

function endReadableNT(state: ReadableState, stream: Readable) {
  if (
    !state.errorEmitted && !state.closeEmitted &&
    !state.endEmitted && state.length === 0
  ) {
    state.endEmitted = true;
    stream.emit("end");

    if (state.autoDestroy) {
      stream.destroy();
    }
  }
}

// Don't raise the hwm > 1GB.
const MAX_HWM = 0x40000000;
function computeNewHighWaterMark(n: number) {
  if (n >= MAX_HWM) {
    n = MAX_HWM;
  } else {
    n--;
    n |= n >>> 1;
    n |= n >>> 2;
    n |= n >>> 4;
    n |= n >>> 8;
    n |= n >>> 16;
    n++;
  }
  return n;
}

function howMuchToRead(n: number, state: ReadableState) {
  if (n <= 0 || (state.length === 0 && state.ended)) {
    return 0;
  }
  if (state.objectMode) {
    return 1;
  }
  if (Number.isNaN(n)) {
    // Only flow one buffer at a time.
    if (state.flowing && state.length) {
      return state.buffer.first().length;
    }
    return state.length;
  }
  if (n <= state.length) {
    return n;
  }
  return state.ended ? state.length : 0;
}

function onEofChunk(stream: Readable, state: ReadableState) {
  if (state.ended) return;
  if (state.decoder) {
    const chunk = state.decoder.end();
    if (chunk && chunk.length) {
      state.buffer.push(chunk);
      state.length += state.objectMode ? 1 : chunk.length;
    }
  }
  state.ended = true;

  if (state.sync) {
    emitReadable(stream);
  } else {
    state.needReadable = false;
    state.emittedReadable = true;
    emitReadable_(stream);
  }
}

function emitReadable(stream: Readable) {
  const state = stream._readableState;
  state.needReadable = false;
  if (!state.emittedReadable) {
    state.emittedReadable = true;
    queueMicrotask(() => emitReadable_(stream));
  }
}

function emitReadable_(stream: Readable) {
  const state = stream._readableState;
  if (!state.destroyed && !state.errored && (state.length || state.ended)) {
    stream.emit("readable");
    state.emittedReadable = false;
  }

  state.needReadable = !state.flowing &&
    !state.ended &&
    state.length <= state.highWaterMark;
  flow(stream);
}

function maybeReadMore(stream: Readable, state: ReadableState) {
  if (!state.readingMore && state.constructed) {
    state.readingMore = true;
    queueMicrotask(() => maybeReadMore_(stream, state));
  }
}

function maybeReadMore_(stream: Readable, state: ReadableState) {
  while (
    !state.reading && !state.ended &&
    (state.length < state.highWaterMark ||
      (state.flowing && state.length === 0))
  ) {
    const len = state.length;
    stream.read(0);
    if (len === state.length) {
      // Didn't get any data, stop spinning.
      break;
    }
  }
  state.readingMore = false;
}

export interface ReadableOptions {
  autoDestroy?: boolean;
  construct?: () => void;
  //TODO(Soremwar)
  //Import available encodings
  defaultEncoding?: string;
  destroy?(
    this: Readable,
    error: Error | null,
    callback: (error: Error | null) => void,
  ): void;
  emitClose?: boolean;
  //TODO(Soremwar)
  //Import available encodings
  encoding?: string;
  highWaterMark?: number;
  objectMode?: boolean;
  read?(this: Readable): void;
}

export class ReadableState {
  [kPaused]: boolean | null = null;
  awaitDrainWriters: Writable | Set<Writable> | null = null;
  buffer = new BufferList();
  closed = false;
  closeEmitted = false;
  constructed: boolean;
  decoder: StringDecoder | null = null;
  destroyed = false;
  emittedReadable = false;
  //TODO(Soremwar)
  //Import available encodings
  encoding: string | null = null;
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
  pipes: Writable[] = [];
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
  _construct?: (cb: (error?: Error) => void) => void;
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
      if (typeof options.construct === "function") {
        this._construct = options.construct;
      }
    }
    this._readableState = new ReadableState(options);

    construct(this, () => {
      maybeReadMore(this, this._readableState);
    });
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

  _read() {
    throw new ERR_METHOD_NOT_IMPLEMENTED("_read()");
  }

  //TODO(Soremwar)
  //Should be duplex
  pipe<T extends Writable>(dest: T, pipeOpts?: { end?: boolean }): T {
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
            (state.awaitDrainWriters as Set<Writable>).add(dest);
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
        //TODO(Soremwar)
        //Should be const s = dest._writableState || dest._readableState;
        const s = dest._writableState;
        if (s && !s.errorEmitted) {
          // User incorrectly emitted 'error' directly on the stream.
          errorOrDestroyDuplex(dest, er);
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
      src.unpipe(dest);
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

  //TODO(Soremwar)
  //Replace string with encoding types
  setEncoding(enc: string) {
    const decoder = new StringDecoder(enc);
    this._readableState.decoder = decoder;
    this._readableState.encoding = this._readableState.decoder.encoding;

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

  destroy(err?: Error, cb?: () => void) {
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

  //TODO(Soremwar)
  //Same deal, string => encodings
  // deno-lint-ignore no-explicit-any
  push(chunk: any, encoding?: string): boolean {
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

Object.defineProperties(Stream, {
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
