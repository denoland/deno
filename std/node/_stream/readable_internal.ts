// Copyright Node.js contributors. All rights reserved. MIT License.
import {
  ERR_STREAM_PUSH_AFTER_EOF,
  ERR_STREAM_UNSHIFT_AFTER_END_EVENT,
} from "../_errors.ts";
import { Buffer } from "../buffer.ts";
import type EventEmitter from "../events.ts";
import type Duplex from "./duplex.ts";
import type Readable from "./readable.ts";
import type { ReadableState } from "./readable.ts";
import { kPaused } from "./symbols.ts";
import type Writable from "./writable.ts";

export function _destroy(
  self: Readable,
  err?: Error | null,
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

export function addChunk(
  stream: Duplex | Readable,
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

// Don't raise the hwm > 1GB.
const MAX_HWM = 0x40000000;
export function computeNewHighWaterMark(n: number) {
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

export function emitReadable(stream: Duplex | Readable) {
  const state = stream._readableState;
  state.needReadable = false;
  if (!state.emittedReadable) {
    state.emittedReadable = true;
    queueMicrotask(() => emitReadable_(stream));
  }
}

function emitReadable_(stream: Duplex | Readable) {
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

export function endReadable(stream: Readable) {
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

export function errorOrDestroy(
  stream: Duplex | Readable,
  err: Error,
  sync = false,
) {
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

function flow(stream: Duplex | Readable) {
  const state = stream._readableState;
  while (state.flowing && stream.read() !== null);
}

/** Pluck off n bytes from an array of buffers.
* Length is the combined lengths of all the buffers in the list.
* This function is designed to be inlinable, so please take care when making
* changes to the function body.
*/
export function fromList(n: number, state: ReadableState) {
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

export function howMuchToRead(n: number, state: ReadableState) {
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

export function maybeReadMore(stream: Readable, state: ReadableState) {
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

export function nReadingNextTick(self: Duplex | Readable) {
  self.read(0);
}

export function onEofChunk(stream: Duplex | Readable, state: ReadableState) {
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

export function pipeOnDrain(src: Duplex | Readable, dest: Duplex | Writable) {
  return function pipeOnDrainFunctionResult() {
    const state = src._readableState;

    if (state.awaitDrainWriters === dest) {
      state.awaitDrainWriters = null;
    } else if (state.multiAwaitDrain) {
      (state.awaitDrainWriters as Set<Duplex | Writable>).delete(dest);
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

export function prependListener(
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

export function readableAddChunk(
  stream: Duplex | Readable,
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

export function resume(stream: Duplex | Readable, state: ReadableState) {
  if (!state.resumeScheduled) {
    state.resumeScheduled = true;
    queueMicrotask(() => resume_(stream, state));
  }
}

function resume_(stream: Duplex | Readable, state: ReadableState) {
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

export function updateReadableListening(self: Duplex | Readable) {
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
