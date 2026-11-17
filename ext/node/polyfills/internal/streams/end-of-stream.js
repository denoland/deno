// deno-lint-ignore-file
// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const lazyProcess = core.createLazyLoader("node:process");
const imported1 = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { kEmptyObject, once } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);
const {
  validateAbortSignal,
  validateBoolean,
  validateFunction,
  validateObject,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");

const { Symbol, ReflectApply } = primordials;

const {
  isClosed,
  isNodeStream,
  isReadable,
  isReadableErrored,
  isReadableFinished,
  isReadableNodeStream,
  isReadableStream,
  isTransformStream,
  isWritable,
  isWritableErrored,
  isWritableFinished,
  isWritableNodeStream,
  isWritableStream,
  kIsClosedPromise,
  willEmitClose: _willEmitClose,
} = core.loadExtScript("ext:deno_node/internal/streams/utils.js");

const _mod2 = core.loadExtScript(
  "ext:deno_node/internal/events/abort_listener.mjs",
);

const {
  AbortError,
  codes: {
    ERR_INVALID_ARG_TYPE,
    ERR_STREAM_PREMATURE_CLOSE,
  },
} = imported1;

// Ported from https://github.com/mafintosh/end-of-stream with
// permission from the author, Mathias Buus (@mafintosh).

"use strict";

const {
  Promise,
  PromisePrototypeThen,
  SymbolDispose,
} = primordials;

let addAbortListener;

function isRequest(stream) {
  return stream.setHeader && typeof stream.abort === "function";
}

const nop = () => {};

/**
 * Returns the current stream error tracked by eos(), if any.
 * @param {import('stream').Stream} stream
 * @returns {Error | null}
 */
function getEosErrored(stream) {
  const errored = isWritableErrored(stream) || isReadableErrored(stream);
  return typeof errored !== "boolean" && errored || null;
}

/**
 * Returns the error eos() would report from an immediate close, including
 * premature close detection for unfinished readable or writable sides.
 * @param {import('stream').Stream} stream
 * @param {boolean} readable
 * @param {boolean | null} readableFinished
 * @param {boolean} writable
 * @param {boolean | null} writableFinished
 * @returns {Error | null}
 */
function getEosOnCloseError(
  stream,
  readable,
  readableFinished,
  writable,
  writableFinished,
) {
  const errored = getEosErrored(stream);
  if (errored) {
    return errored;
  }

  if (readable && !readableFinished && isReadableNodeStream(stream, true)) {
    if (!isReadableFinished(stream, false)) {
      return new ERR_STREAM_PREMATURE_CLOSE();
    }
  }

  if (writable && !writableFinished) {
    if (!isWritableFinished(stream, false)) {
      return new ERR_STREAM_PREMATURE_CLOSE();
    }
  }

  return null;
}

const kEosNodeSynchronousCallback = Symbol("kEosNodeSynchronousCallback");

function eos(stream, options, callback) {
  if (arguments.length === 2) {
    callback = options;
    options = kEmptyObject;
  } else if (options == null) {
    options = kEmptyObject;
  } else {
    validateObject(options, "options");
  }
  validateFunction(callback, "callback");
  validateAbortSignal(options.signal, "options.signal");

  // Capture the current async context so that the callback runs in the
  // same AsyncLocalStorage scope that was active when eos() was called.
  // In Node.js this happens automatically through the native AsyncWrap
  // layer, but Deno's ops don't propagate Node-style async context.
  const snapshot = core.getAsyncContext();
  const originalCallback = callback;
  callback = function (...args) {
    const previousContext = core.getAsyncContext();
    try {
      core.setAsyncContext(snapshot);
      return originalCallback.apply(this, args);
    } finally {
      core.setAsyncContext(previousContext);
    }
  };

  callback = once(callback);

  if (isReadableStream(stream) || isWritableStream(stream)) {
    return eosWeb(stream, options, callback);
  }

  // Only a genuine web TransformStream (whose `.readable` is a real
  // ReadableStream carrying `kIsClosedPromise`) is tracked here; a loosely
  // shaped `{ readable, writable }` object falls through to the
  // ERR_INVALID_ARG_TYPE below rather than a cryptic property-access TypeError.
  if (isTransformStream(stream) && isReadableStream(stream.readable)) {
    // A web TransformStream has no `kIsClosedPromise` of its own; track its
    // readable side, which closes once the transform's output is fully done.
    // Used by `addAbortSignal` to clean up its abort listener.
    return eosWeb(stream.readable, options, callback);
  }

  if (!isNodeStream(stream)) {
    throw new ERR_INVALID_ARG_TYPE("stream", [
      "ReadableStream",
      "WritableStream",
      "Stream",
    ], stream);
  }

  const readable = options.readable ?? isReadableNodeStream(stream);
  const writable = options.writable ?? isWritableNodeStream(stream);

  const wState = stream._writableState;
  const rState = stream._readableState;

  let readableFinished = isReadableFinished(stream, false);
  let writableFinished = isWritableFinished(stream, false);

  // TODO (ronag): Improve soft detection to include core modules and
  // common ecosystem modules that do properly emit 'close' but fail
  // this generic check.
  let willEmitClose = _willEmitClose(stream) &&
    isReadableNodeStream(stream) === readable &&
    isWritableNodeStream(stream) === writable;

  // undefined: not settled yet, null: settled OK, Error: settled with error
  let immediateResult;
  if (isClosed(stream)) {
    immediateResult = getEosOnCloseError(
      stream,
      readable,
      readableFinished,
      writable,
      writableFinished,
    );
  } else if (wState?.errorEmitted || rState?.errorEmitted) {
    if (!willEmitClose) immediateResult = getEosErrored(stream);
  } else if (
    !readable &&
    (!willEmitClose || isReadable(stream)) &&
    (writableFinished || isWritable(stream) === false) &&
    (wState == null || wState.pendingcb === undefined || wState.pendingcb === 0)
  ) {
    immediateResult = getEosErrored(stream);
  } else if (
    !writable &&
    (!willEmitClose || isWritable(stream)) &&
    (readableFinished || isReadable(stream) === false)
  ) {
    immediateResult = getEosErrored(stream);
  } else if (rState && stream.req && stream.aborted) {
    immediateResult = getEosErrored(stream);
  }

  let cleanup = () => {
    callback = nop;
  };
  if (immediateResult !== undefined) {
    if (options.error !== false) {
      stream.on("error", nop);
      cleanup = () => {
        callback = nop;
        stream.removeListener("error", nop);
      };
    }
  } else if (options.signal?.aborted) {
    immediateResult = new AbortError(undefined, {
      cause: options.signal.reason,
    });
  }

  if (immediateResult !== undefined && options[kEosNodeSynchronousCallback]) {
    ReflectApply(
      callback,
      stream,
      immediateResult === null ? [] : [immediateResult],
    );
    return cleanup;
  }

  if (immediateResult !== undefined) {
    lazyProcess().nextTick(() =>
      ReflectApply(
        callback,
        stream,
        immediateResult === null ? [] : [immediateResult],
      )
    );
    return cleanup;
  }

  const onlegacyfinish = () => {
    if (!stream.writable) {
      onfinish();
    }
  };

  const onfinish = () => {
    writableFinished = true;
    // Stream should not be destroyed here. If it is that
    // means that user space is doing something differently and
    // we cannot trust willEmitClose.
    if (stream.destroyed) {
      willEmitClose = false;
    }

    if (willEmitClose && (!stream.readable || readable)) {
      return;
    }

    if (!readable || readableFinished) {
      callback.call(stream);
    }
  };

  const onend = () => {
    readableFinished = true;
    // Stream should not be destroyed here. If it is that
    // means that user space is doing something differently and
    // we cannot trust willEmitClose.
    if (stream.destroyed) {
      willEmitClose = false;
    }

    if (willEmitClose && (!stream.writable || writable)) {
      return;
    }

    if (!writable || writableFinished) {
      callback.call(stream);
    }
  };

  const onerror = (err) => {
    callback.call(stream, err);
  };

  const onclose = () => {
    const error = getEosOnCloseError(
      stream,
      readable,
      readableFinished,
      writable,
      writableFinished,
    );
    if (error === null) {
      callback.call(stream);
    } else {
      callback.call(stream, error);
    }
  };

  const onrequest = () => {
    stream.req.on("finish", onfinish);
  };

  if (isRequest(stream)) {
    stream.on("complete", onfinish);
    if (!willEmitClose) {
      stream.on("abort", onclose);
    }
    if (stream.req) {
      onrequest();
    } else {
      stream.on("request", onrequest);
    }
  } else if (writable && !wState) { // legacy streams
    stream.on("end", onlegacyfinish);
    stream.on("close", onlegacyfinish);
  }

  // Not all streams will emit 'close' after 'aborted'.
  if (!willEmitClose && typeof stream.aborted === "boolean") {
    stream.on("aborted", onclose);
  }

  stream.on("end", onend);
  stream.on("finish", onfinish);
  if (options.error !== false) {
    stream.on("error", onerror);
  }
  stream.on("close", onclose);

  cleanup = () => {
    callback = nop;
    stream.removeListener("aborted", onclose);
    stream.removeListener("complete", onfinish);
    stream.removeListener("abort", onclose);
    stream.removeListener("request", onrequest);
    if (stream.req) stream.req.removeListener("finish", onfinish);
    stream.removeListener("end", onlegacyfinish);
    stream.removeListener("close", onlegacyfinish);
    stream.removeListener("finish", onfinish);
    stream.removeListener("end", onend);
    stream.removeListener("error", onerror);
    stream.removeListener("close", onclose);
  };

  if (options.signal) {
    const abort = () => {
      // Keep it because cleanup removes it.
      const endCallback = callback;
      cleanup();
      endCallback.call(
        stream,
        new AbortError(undefined, { cause: options.signal.reason }),
      );
    };
    addAbortListener ??= _mod2.addAbortListener;
    const disposable = addAbortListener(options.signal, abort);
    const originalCallback = callback;
    callback = once((...args) => {
      disposable[SymbolDispose]();
      originalCallback.apply(stream, args);
    });
  }

  return cleanup;
}

function eosWeb(stream, options, callback) {
  let isAborted = false;
  let abort = nop;
  let disposable;
  const cleanup = () => {
    callback = nop;
    disposable?.[SymbolDispose]();
    disposable = undefined;
  };
  if (options.signal) {
    abort = () => {
      isAborted = true;
      callback.call(
        stream,
        new AbortError(undefined, { cause: options.signal.reason }),
      );
    };
    if (options.signal.aborted) {
      lazyProcess().nextTick(abort);
    } else {
      addAbortListener ??= _mod2.addAbortListener;
      disposable = addAbortListener(options.signal, abort);
      const originalCallback = callback;
      callback = once((...args) => {
        cleanup();
        originalCallback.apply(stream, args);
      });
    }
  }
  const resolverFn = (...args) => {
    if (!isAborted) {
      lazyProcess().nextTick(() => callback.apply(stream, args));
    }
  };
  PromisePrototypeThen(
    stream[kIsClosedPromise].promise,
    resolverFn,
    resolverFn,
  );
  // Deno diverges from upstream, which returns `nop`, to honor the returned
  // cleanup function contract.
  // https://github.com/nodejs/node/pull/46205
  return cleanup;
}

function finished(stream, opts) {
  let autoCleanup = false;
  if (opts === null) {
    opts = kEmptyObject;
  }
  if (opts?.cleanup) {
    validateBoolean(opts.cleanup, "cleanup");
    autoCleanup = opts.cleanup;
  }
  return new Promise((resolve, reject) => {
    const cleanup = eos(stream, opts, (err) => {
      if (autoCleanup) {
        cleanup();
      }
      if (err) {
        reject(err);
      } else {
        resolve();
      }
    });
  });
}

return {
  finished,
  eos,
  default: eos,
  kEosNodeSynchronousCallback,
};
})();
