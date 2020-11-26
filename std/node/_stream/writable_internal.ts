// Copyright Node.js contributors. All rights reserved. MIT License.
import type Duplex from "./duplex.ts";
import type Writable from "./writable.ts";
import type { WritableState } from "./writable.ts";
import { kDestroy } from "./symbols.ts";
import { ERR_MULTIPLE_CALLBACK, ERR_STREAM_DESTROYED } from "../_errors.ts";

export type writeV = (
  // deno-lint-ignore no-explicit-any
  chunks: Array<{ chunk: any; encoding: string }>,
  callback: (error?: Error | null) => void,
) => void;

export type AfterWriteTick = {
  cb: (error?: Error) => void;
  count: number;
  state: WritableState;
  stream: Writable;
};

export const kOnFinished = Symbol("kOnFinished");

function _destroy(
  self: Writable,
  err?: Error | null,
  cb?: (error?: Error | null) => void,
) {
  self._destroy(err || null, (err) => {
    const w = self._writableState;

    if (err) {
      // Avoid V8 leak, https://github.com/nodejs/node/pull/34103#issuecomment-652002364
      err.stack;

      if (!w.errored) {
        w.errored = err;
      }
    }

    w.closed = true;

    if (typeof cb === "function") {
      cb(err);
    }

    if (err) {
      queueMicrotask(() => {
        if (!w.errorEmitted) {
          w.errorEmitted = true;
          self.emit("error", err);
        }
        w.closeEmitted = true;
        if (w.emitClose) {
          self.emit("close");
        }
      });
    } else {
      queueMicrotask(() => {
        w.closeEmitted = true;
        if (w.emitClose) {
          self.emit("close");
        }
      });
    }
  });
}

export function afterWrite(
  stream: Writable,
  state: WritableState,
  count: number,
  cb: (error?: Error) => void,
) {
  const needDrain = !state.ending && !stream.destroyed && state.length === 0 &&
    state.needDrain;
  if (needDrain) {
    state.needDrain = false;
    stream.emit("drain");
  }

  while (count-- > 0) {
    state.pendingcb--;
    cb();
  }

  if (state.destroyed) {
    errorBuffer(state);
  }

  finishMaybe(stream, state);
}

export function afterWriteTick({
  cb,
  count,
  state,
  stream,
}: AfterWriteTick) {
  state.afterWriteTickInfo = null;
  return afterWrite(stream, state, count, cb);
}

/** If there's something in the buffer waiting, then process it.*/
export function clearBuffer(stream: Duplex | Writable, state: WritableState) {
  if (
    state.corked ||
    state.bufferProcessing ||
    state.destroyed ||
    !state.constructed
  ) {
    return;
  }

  const { buffered, bufferedIndex, objectMode } = state;
  const bufferedLength = buffered.length - bufferedIndex;

  if (!bufferedLength) {
    return;
  }

  const i = bufferedIndex;

  state.bufferProcessing = true;
  if (bufferedLength > 1 && stream._writev) {
    state.pendingcb -= bufferedLength - 1;

    const callback = state.allNoop ? nop : (err: Error) => {
      for (let n = i; n < buffered.length; ++n) {
        buffered[n].callback(err);
      }
    };
    const chunks = state.allNoop && i === 0 ? buffered : buffered.slice(i);

    doWrite(stream, state, true, state.length, chunks, "", callback);

    resetBuffer(state);
  } else {
    do {
      const { chunk, encoding, callback } = buffered[i];
      const len = objectMode ? 1 : chunk.length;
      doWrite(stream, state, false, len, chunk, encoding, callback);
    } while (i < buffered.length && !state.writing);

    if (i === buffered.length) {
      resetBuffer(state);
    } else if (i > 256) {
      buffered.splice(0, i);
      state.bufferedIndex = 0;
    } else {
      state.bufferedIndex = i;
    }
  }
  state.bufferProcessing = false;
}

export function destroy(this: Writable, err?: Error | null, cb?: () => void) {
  const w = this._writableState;

  if (w.destroyed) {
    if (typeof cb === "function") {
      cb();
    }

    return this;
  }

  if (err) {
    // Avoid V8 leak, https://github.com/nodejs/node/pull/34103#issuecomment-652002364
    err.stack;

    if (!w.errored) {
      w.errored = err;
    }
  }

  w.destroyed = true;

  if (!w.constructed) {
    this.once(kDestroy, (er) => {
      _destroy(this, err || er, cb);
    });
  } else {
    _destroy(this, err, cb);
  }

  return this;
}

function doWrite(
  stream: Duplex | Writable,
  state: WritableState,
  writev: boolean,
  len: number,
  // deno-lint-ignore no-explicit-any
  chunk: any,
  encoding: string,
  cb: (error: Error) => void,
) {
  state.writelen = len;
  state.writecb = cb;
  state.writing = true;
  state.sync = true;
  if (state.destroyed) {
    state.onwrite(new ERR_STREAM_DESTROYED("write"));
  } else if (writev) {
    (stream._writev as unknown as writeV)(chunk, state.onwrite);
  } else {
    stream._write(chunk, encoding, state.onwrite);
  }
  state.sync = false;
}

/** If there's something in the buffer waiting, then invoke callbacks.*/
export function errorBuffer(state: WritableState) {
  if (state.writing) {
    return;
  }

  for (let n = state.bufferedIndex; n < state.buffered.length; ++n) {
    const { chunk, callback } = state.buffered[n];
    const len = state.objectMode ? 1 : chunk.length;
    state.length -= len;
    callback(new ERR_STREAM_DESTROYED("write"));
  }

  for (const callback of state[kOnFinished].splice(0)) {
    callback(new ERR_STREAM_DESTROYED("end"));
  }

  resetBuffer(state);
}

export function errorOrDestroy(stream: Writable, err: Error, sync = false) {
  const w = stream._writableState;

  if (w.destroyed) {
    return stream;
  }

  if (w.autoDestroy) {
    stream.destroy(err);
  } else if (err) {
    // Avoid V8 leak, https://github.com/nodejs/node/pull/34103#issuecomment-652002364
    err.stack;

    if (!w.errored) {
      w.errored = err;
    }
    if (sync) {
      queueMicrotask(() => {
        if (w.errorEmitted) {
          return;
        }
        w.errorEmitted = true;
        stream.emit("error", err);
      });
    } else {
      if (w.errorEmitted) {
        return;
      }
      w.errorEmitted = true;
      stream.emit("error", err);
    }
  }
}

function finish(stream: Writable, state: WritableState) {
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
  stream: Writable,
  state: WritableState,
  sync?: boolean,
) {
  if (needFinish(state)) {
    prefinish(stream, state);
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

export function needFinish(state: WritableState) {
  return (state.ending &&
    state.constructed &&
    state.length === 0 &&
    !state.errored &&
    state.buffered.length === 0 &&
    !state.finished &&
    !state.writing);
}

export function nop() {}

export function resetBuffer(state: WritableState) {
  state.buffered = [];
  state.bufferedIndex = 0;
  state.allBuffers = true;
  state.allNoop = true;
}

function onwriteError(
  stream: Writable,
  state: WritableState,
  er: Error,
  cb: (error: Error) => void,
) {
  --state.pendingcb;

  cb(er);
  errorBuffer(state);
  errorOrDestroy(stream, er);
}

export function onwrite(stream: Writable, er?: Error | null) {
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
          stream,
          state,
        };
        queueMicrotask(() =>
          afterWriteTick(state.afterWriteTickInfo as AfterWriteTick)
        );
      }
    } else {
      afterWrite(stream, state, 1, cb as (error?: Error) => void);
    }
  }
}

export function prefinish(stream: Writable, state: WritableState) {
  if (!state.prefinished && !state.finalCalled) {
    if (typeof stream._final === "function" && !state.destroyed) {
      state.finalCalled = true;

      state.sync = true;
      state.pendingcb++;
      stream._final((err) => {
        state.pendingcb--;
        if (err) {
          for (const callback of state[kOnFinished].splice(0)) {
            callback(err);
          }
          errorOrDestroy(stream, err, state.sync);
        } else if (needFinish(state)) {
          state.prefinished = true;
          stream.emit("prefinish");
          state.pendingcb++;
          queueMicrotask(() => finish(stream, state));
        }
      });
      state.sync = false;
    } else {
      state.prefinished = true;
      stream.emit("prefinish");
    }
  }
}

export function writeOrBuffer(
  stream: Duplex | Writable,
  state: WritableState,
  // deno-lint-ignore no-explicit-any
  chunk: any,
  encoding: string,
  callback: (error: Error) => void,
) {
  const len = state.objectMode ? 1 : chunk.length;

  state.length += len;

  if (state.writing || state.corked || state.errored || !state.constructed) {
    state.buffered.push({ chunk, encoding, callback });
    if (state.allBuffers && encoding !== "buffer") {
      state.allBuffers = false;
    }
    if (state.allNoop && callback !== nop) {
      state.allNoop = false;
    }
  } else {
    state.writelen = len;
    state.writecb = callback;
    state.writing = true;
    state.sync = true;
    stream._write(chunk, encoding, state.onwrite);
    state.sync = false;
  }

  const ret = state.length < state.highWaterMark;

  if (!ret) {
    state.needDrain = true;
  }

  return ret && !state.errored && !state.destroyed;
}
