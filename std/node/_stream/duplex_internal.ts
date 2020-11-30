// Copyright Node.js contributors. All rights reserved. MIT License.
import type { ReadableState } from "./readable.ts";
import { addChunk, maybeReadMore, onEofChunk } from "./readable_internal.ts";
import type Writable from "./writable.ts";
import type { WritableState } from "./writable.ts";
import {
  afterWrite,
  AfterWriteTick,
  afterWriteTick,
  clearBuffer,
  errorBuffer,
  kOnFinished,
  needFinish,
  prefinish,
} from "./writable_internal.ts";
import { Buffer } from "../buffer.ts";
import type Duplex from "./duplex.ts";
import {
  ERR_MULTIPLE_CALLBACK,
  ERR_STREAM_PUSH_AFTER_EOF,
  ERR_STREAM_UNSHIFT_AFTER_END_EVENT,
} from "../_errors.ts";

export function endDuplex(stream: Duplex) {
  const state = stream._readableState;

  if (!state.endEmitted) {
    state.ended = true;
    queueMicrotask(() => endReadableNT(state, stream));
  }
}

function endReadableNT(state: ReadableState, stream: Duplex) {
  // Check that we didn't get one last unshift.
  if (
    !state.errorEmitted && !state.closeEmitted &&
    !state.endEmitted && state.length === 0
  ) {
    state.endEmitted = true;
    stream.emit("end");

    if (stream.writable && stream.allowHalfOpen === false) {
      queueMicrotask(() => endWritableNT(state, stream));
    } else if (state.autoDestroy) {
      // In case of duplex streams we need a way to detect
      // if the writable side is ready for autoDestroy as well.
      const wState = stream._writableState;
      const autoDestroy = !wState || (
        wState.autoDestroy &&
        // We don't expect the writable to ever 'finish'
        // if writable is explicitly set to false.
        (wState.finished || wState.writable === false)
      );

      if (autoDestroy) {
        stream.destroy();
      }
    }
  }
}

function endWritableNT(state: ReadableState, stream: Duplex) {
  const writable = stream.writable &&
    !stream.writableEnded &&
    !stream.destroyed;
  if (writable) {
    stream.end();
  }
}

export function errorOrDestroy(
  // deno-lint-ignore no-explicit-any
  this: any,
  stream: Duplex,
  err: Error,
  sync = false,
) {
  const r = stream._readableState;
  const w = stream._writableState;

  if (w.destroyed || r.destroyed) {
    return this;
  }

  if (r.autoDestroy || w.autoDestroy) {
    stream.destroy(err);
  } else if (err) {
    // Avoid V8 leak, https://github.com/nodejs/node/pull/34103#issuecomment-652002364
    err.stack;

    if (w && !w.errored) {
      w.errored = err;
    }
    if (r && !r.errored) {
      r.errored = err;
    }

    if (sync) {
      queueMicrotask(() => {
        if (w.errorEmitted || r.errorEmitted) {
          return;
        }

        w.errorEmitted = true;
        r.errorEmitted = true;

        stream.emit("error", err);
      });
    } else {
      if (w.errorEmitted || r.errorEmitted) {
        return;
      }

      w.errorEmitted = true;
      r.errorEmitted = true;

      stream.emit("error", err);
    }
  }
}

function finish(stream: Duplex, state: WritableState) {
  state.pendingcb--;
  if (state.errorEmitted || state.closeEmitted) {
    return;
  }

  state.finished = true;

  for (const callback of state[kOnFinished].splice(0)) {
    callback();
  }

  stream.emit("finish");

  if (state.autoDestroy) {
    stream.destroy();
  }
}

export function finishMaybe(
  stream: Duplex,
  state: WritableState,
  sync?: boolean,
) {
  if (needFinish(state)) {
    prefinish(stream as Writable, state);
    if (state.pendingcb === 0 && needFinish(state)) {
      state.pendingcb++;
      if (sync) {
        queueMicrotask(() => finish(stream, state));
      } else {
        finish(stream, state);
      }
    }
  }
}

export function onwrite(stream: Duplex, er?: Error | null) {
  const state = stream._writableState;
  const sync = state.sync;
  const cb = state.writecb;

  if (typeof cb !== "function") {
    errorOrDestroy(stream, new ERR_MULTIPLE_CALLBACK());
    return;
  }

  state.writing = false;
  state.writecb = null;
  state.length -= state.writelen;
  state.writelen = 0;

  if (er) {
    // Avoid V8 leak, https://github.com/nodejs/node/pull/34103#issuecomment-652002364
    er.stack;

    if (!state.errored) {
      state.errored = er;
    }

    if (stream._readableState && !stream._readableState.errored) {
      stream._readableState.errored = er;
    }

    if (sync) {
      queueMicrotask(() => onwriteError(stream, state, er, cb));
    } else {
      onwriteError(stream, state, er, cb);
    }
  } else {
    if (state.buffered.length > state.bufferedIndex) {
      clearBuffer(stream, state);
    }

    if (sync) {
      if (
        state.afterWriteTickInfo !== null &&
        state.afterWriteTickInfo.cb === cb
      ) {
        state.afterWriteTickInfo.count++;
      } else {
        state.afterWriteTickInfo = {
          count: 1,
          cb: (cb as (error?: Error) => void),
          stream: stream as Writable,
          state,
        };
        queueMicrotask(() =>
          afterWriteTick(state.afterWriteTickInfo as AfterWriteTick)
        );
      }
    } else {
      afterWrite(stream as Writable, state, 1, cb as (error?: Error) => void);
    }
  }
}

function onwriteError(
  stream: Duplex,
  state: WritableState,
  er: Error,
  cb: (error: Error) => void,
) {
  --state.pendingcb;

  cb(er);
  errorBuffer(state);
  errorOrDestroy(stream, er);
}

export function readableAddChunk(
  stream: Duplex,
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
