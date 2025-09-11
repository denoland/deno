// deno-lint-ignore-file
// Copyright 2018-2025 the Deno authors. MIT license.

import process from "node:process";
import { primordials } from "ext:core/mod.js";
import eos from "ext:deno_node/internal/streams/end-of-stream.js";
import { once } from "ext:deno_node/internal/util.mjs";
import destroyImpl from "ext:deno_node/internal/streams/destroy.js";
import Duplex from "node:_stream_duplex";
import imported1 from "ext:deno_node/internal/errors.ts";
import {
  validateAbortSignal,
  validateFunction,
} from "ext:deno_node/internal/validators.mjs";

import {
  isIterable,
  isNodeStream,
  isReadable,
  isReadableFinished,
  isReadableNodeStream,
  isReadableStream,
  isTransformStream,
  isWebStream,
} from "ext:deno_node/internal/streams/utils.js";

import { AbortController } from "ext:deno_web/03_abort_signal.js";
import _mod3 from "node:_stream_readable";
import * as _mod4 from "ext:deno_node/internal/events/abort_listener.mjs";
import _mod5 from "node:_stream_passthrough";

const {
  AbortError,
  aggregateTwoErrors,
  codes: {
    ERR_INVALID_ARG_TYPE,
    ERR_INVALID_RETURN_VALUE,
    ERR_MISSING_ARGS,
    ERR_STREAM_DESTROYED,
    ERR_STREAM_PREMATURE_CLOSE,
    ERR_STREAM_UNABLE_TO_PIPE,
  },
} = imported1;

// Ported from https://github.com/mafintosh/pump with
// permission from the author, Mathias Buus (@mafintosh).

"use strict";

const {
  ArrayIsArray,
  Promise,
  SymbolAsyncIterator,
  SymbolDispose,
} = primordials;

let PassThrough;
let Readable;
let addAbortListener;

// Feature detection functions - more robust than constructor.name checks
function isClientRequest(stream) {
  return stream &&
    typeof stream.setHeader === "function" &&
    typeof stream.abort === "function" &&
    typeof stream.path === "string";
}

function isServerResponse(stream) {
  return stream &&
    typeof stream.setHeader === "function" &&
    typeof stream.writeHead === "function" &&
    typeof stream.statusCode === "number";
}

function isIncomingMessage(stream) {
  return stream &&
    typeof stream.headers === "object" &&
    (typeof stream.method === "string" ||
      typeof stream.statusCode === "number");
}

function isHTTPStream(stream) {
  return isClientRequest(stream) || isServerResponse(stream);
}

function destroyer(stream, reading, writing) {
  let finished = false;
  stream.on("close", () => {
    finished = true;
  });

  const cleanup = eos(
    stream,
    { readable: reading, writable: writing },
    (err) => {
      finished = !err;
    },
  );

  return {
    destroy: (err) => {
      if (finished) return;
      finished = true;
      const isReq = stream?.setHeader && typeof stream.abort === "function";
      destroyImpl.destroyer(
        stream,
        err || (isReq ? null : new ERR_STREAM_DESTROYED("pipe")),
      );
    },
    cleanup,
  };
}

function popCallback(streams) {
  // Streams should never be an empty array. It should always contain at least
  // a single stream. Therefore optimize for the average case instead of
  // checking for length === 0 as well.
  validateFunction(streams[streams.length - 1], "streams[stream.length - 1]");
  return streams.pop();
}

function makeAsyncIterable(val) {
  if (isIterable(val)) {
    return val;
  } else if (isReadableNodeStream(val)) {
    // Legacy streams are not Iterable.
    return fromReadable(val);
  }
  throw new ERR_INVALID_ARG_TYPE(
    "val",
    ["Readable", "Iterable", "AsyncIterable"],
    val,
  );
}

async function* fromReadable(val) {
  Readable ??= _mod3;
  try {
    for await (
      const chunk of Readable.prototype[SymbolAsyncIterator].call(val)
    ) {
      yield chunk;
    }
  } catch (err) {
    if (err.code === "ERR_STREAM_PREMATURE_CLOSE" || val.destroyed) {
      return;
    }
    throw err;
  }
}

async function pumpToHTTPClientRequest(
  iterable,
  clientRequest,
  finish,
  { end },
) {
  let error;
  let finished = false;

  const safeFinish = (err) => {
    if (finished) return;
    finished = true;
    finish(err);
  };

  const cleanup = eos(clientRequest, { readable: false }, (err) => {
    // Only finish if it's not a normal completion
    if (err && err.code !== "ERR_STREAM_PREMATURE_CLOSE") {
      safeFinish(err);
    }
  });

  try {
    // Wait for socket connection if needed
    if (!clientRequest.socket) {
      await new Promise((resolve) => {
        clientRequest.once("socket", resolve);
      });
    }

    if (clientRequest.socket.connecting) {
      await new Promise((resolve) => {
        clientRequest.socket.once("connect", resolve);
      });
    }

    // Check if there's buffered data and the stream needs draining
    if (
      clientRequest.outputData && clientRequest.outputData.length > 0 &&
      clientRequest.writableNeedDrain
    ) {
      // Wait for drain instead of forcing flush
      await new Promise((resolve) => {
        clientRequest.once("drain", resolve);
      });
    }

    try {
      for await (const chunk of iterable) {
        // Write directly and handle the buffering manually
        const writeSuccess = clientRequest.write(chunk);

        // If write returns false, wait for drain using writableNeedDrain pattern
        if (!writeSuccess) {
          await new Promise((resolve) => {
            // Use writableNeedDrain for proper backpressure handling
            if (clientRequest.writableNeedDrain) {
              clientRequest.once("drain", resolve);
            } else {
              // Stream is already drained or doesn't support writableNeedDrain
              resolve();
            }
          });
        }
      }
    } catch (iterError) {
      // If the iteration was interrupted due to source destroy, that's not an error for the pipeline
      if (
        iterError.code === "ERR_STREAM_PREMATURE_CLOSE" ||
        iterError.message?.includes("destroyed")
      ) {
      } else {
        throw iterError; // Re-throw unexpected errors
      }
    }

    if (end) {
      clientRequest.end();
    }

    safeFinish();
  } catch (err) {
    safeFinish(err);
  } finally {
    cleanup();
  }
}

async function pumpToNode(iterable, writable, finish, { end }) {
  let error;
  let onresolve = null;

  const resume = (err) => {
    if (err) {
      error = err;
    }

    if (onresolve) {
      const callback = onresolve;
      onresolve = null;
      callback();
    }
  };

  // For HTTP streams, add timeout to drain waits
  const isHTTPStreamDetected = isHTTPStream(writable);

  const wait = () =>
    new Promise((resolve, reject) => {
      if (error) {
        reject(error);
      } else {
        onresolve = () => {
          if (error) {
            reject(error);
          } else {
            resolve();
          }
        };

        // Use writableNeedDrain for proper backpressure detection
        if (isHTTPStreamDetected && !writable.writableNeedDrain) {
          // If HTTP stream doesn't need drain, resolve immediately
          if (onresolve) {
            const callback = onresolve;
            onresolve = null;
            process.nextTick(callback);
          }
        }
      }
    });

  writable.on("drain", resume);
  const cleanup = eos(writable, { readable: false }, resume);

  try {
    if (writable.writableNeedDrain) {
      await wait();
    }

    for await (const chunk of iterable) {
      const writeResult = writable.write(chunk);
      if (!writeResult) {
        await wait();
      }
    }

    if (end) {
      writable.end();
      await wait();
    }

    finish();
  } catch (err) {
    finish(error !== err ? aggregateTwoErrors(error, err) : err);
  } finally {
    cleanup();
    writable.off("drain", resume);
  }
}

async function pumpToWeb(readable, writable, finish, { end }) {
  if (isTransformStream(writable)) {
    writable = writable.writable;
  }
  // https://streams.spec.whatwg.org/#example-manual-write-with-backpressure
  const writer = writable.getWriter();
  try {
    for await (const chunk of readable) {
      await writer.ready;
      writer.write(chunk).catch(() => {});
    }

    await writer.ready;

    if (end) {
      await writer.close();
    }

    finish();
  } catch (err) {
    try {
      await writer.abort(err);
      finish(err);
    } catch (err) {
      finish(err);
    }
  }
}

function pipeline(...streams) {
  return pipelineImpl(streams, once(popCallback(streams)));
}

function pipelineImpl(streams, callback, opts) {
  if (streams.length === 1 && ArrayIsArray(streams[0])) {
    streams = streams[0];
  }

  if (streams.length < 2) {
    throw new ERR_MISSING_ARGS("streams");
  }

  const ac = new AbortController();
  const signal = ac.signal;
  const outerSignal = opts?.signal;

  // Need to cleanup event listeners if last stream is readable
  // https://github.com/nodejs/node/issues/35452
  const lastStreamCleanup = [];

  validateAbortSignal(outerSignal, "options.signal");

  function abort() {
    finishImpl(new AbortError(undefined, { cause: outerSignal?.reason }));
  }

  addAbortListener ??= _mod4.addAbortListener;
  let disposable;
  if (outerSignal) {
    disposable = addAbortListener(outerSignal, abort);
  }

  let error;
  let value;
  const destroys = [];

  let finishCount = 0;

  function finish(err) {
    finishImpl(err, --finishCount === 0);
  }

  function finishOnlyHandleError(err) {
    // Treat any first error as final to invoke callback immediately
    finishImpl(err, true);
  }

  function finishImpl(err, final) {
    if (err && (!error || error.code === "ERR_STREAM_PREMATURE_CLOSE")) {
      error = err;
    }

    if (!error && !final) {
      return;
    }

    // Collect streams that need to be waited for closure
    const streamsToWaitFor = [];
    const destroyFunctions = [];

    // Extract streams from destroyers before calling destroy
    while (destroys.length) {
      const destroyer = destroys.shift();
      if (typeof destroyer === "function") {
        destroyFunctions.push(destroyer);
      } else if (destroyer && typeof destroyer.destroy === "function") {
        destroyFunctions.push(destroyer.destroy);
        // Find the stream associated with this destroyer by checking the closure
        // This is a bit hacky, but we need to track which streams to wait for
      }
    }

    // Call all destroy functions first
    destroyFunctions.forEach((destroy) => destroy(error));

    // Deno-specific async destroy handling
    //
    // Unlike Node.js, Deno sometimes needs to wait for streams to fully close after destroy()
    // due to differences in how the underlying resource cleanup works:
    //
    // - Node.js: Stream destroy is often synchronous, close event fires immediately
    // - Deno: Web API-based resources may require async cleanup (setImmediate, setTimeout)
    //   causing a delay between destroyed=true and the close event
    //
    // This is particularly important for test compatibility where Node.js tests expect
    // specific timing of destroy/close events.
    let hasAsyncDestroy = false;
    if (final && error && streams && streams.length > 0) {
      for (const stream of streams) {
        if (stream && stream.destroyed && !stream.closed) {
          // Detect custom _destroy implementations that differ from base Node.js behavior
          // These typically indicate async resource cleanup requirements
          const hasCustomDestroy = stream._destroy &&
            stream._destroy !== Readable.prototype._destroy;

          if (hasCustomDestroy) {
            hasAsyncDestroy = true;
            break;
          }
        }
      }
    }

    // Only wait for close events if we have streams with async destroy
    if (hasAsyncDestroy) {
      let pendingCloses = 0;
      const onStreamClosed = () => {
        if (--pendingCloses === 0) {
          // All streams closed, now call the callback
          disposable?.[SymbolDispose]();
          ac.abort();
          if (!error) {
            lastStreamCleanup.forEach((fn) => fn());
          }
          process.nextTick(callback, error, value);
        }
      };

      // Set up close listeners only for streams that actually need async destroy coordination
      // This is more targeted than Node.js which doesn't need this level of async coordination
      for (const stream of streams) {
        if (stream && typeof stream.on === "function" && !stream.closed) {
          // Only add close listener if this specific stream has custom async destroy
          // Custom _destroy methods indicate Web API resource cleanup that may be async
          const needsCloseListener = stream._destroy &&
            stream._destroy !== Readable.prototype._destroy;

          if (needsCloseListener) {
            // Increase maxListeners for Deno-specific compatibility requirements
            //
            // Why we need more listeners than Node.js:
            // 1. Async destroy edge case: Deno's stream lifecycle differs from Node.js when
            //    handling custom _destroy methods that use setImmediate/setTimeout.
            //    Node.js streams complete synchronously in many cases where Deno requires
            //    async completion, necessitating additional close event tracking.
            //
            // 2. HTTP stream integration: Deno's HTTP implementation built on Web APIs
            //    has different resource cleanup patterns than Node.js's native HTTP.
            //    We need additional lifecycle coordination between Web streams and Node streams.
            //
            // 3. Pipeline complexity: Complex multi-stream pipelines in Deno can involve
            //    more intermediate state tracking than Node.js's simpler implementation.
            //
            // Node.js typically uses 2-10 close listeners; we need up to ~30 for these edge cases.
            if (stream.setMaxListeners && stream.getMaxListeners() < 30) {
              stream.setMaxListeners(30);
            }
            pendingCloses++;
            stream.once("close", onStreamClosed);
          }
        }
      }

      // Fallback - if no streams to wait for, proceed immediately
      if (pendingCloses === 0) {
        disposable?.[SymbolDispose]();
        ac.abort();
        if (!error) {
          lastStreamCleanup.forEach((fn) => fn());
        }
        process.nextTick(callback, error, value);
      }
    } else {
      // No async destroy streams, proceed as before
      disposable?.[SymbolDispose]();
      ac.abort();

      if (final) {
        if (!error) {
          lastStreamCleanup.forEach((fn) => fn());
        }
        process.nextTick(callback, error, value);
      }
    }
  }

  let ret;
  for (let i = 0; i < streams.length; i++) {
    const stream = streams[i];
    const reading = i < streams.length - 1;
    const writing = i > 0;
    const next = i + 1 < streams.length ? streams[i + 1] : null;
    const end = reading || opts?.end !== false;
    const isLastStream = i === streams.length - 1;

    if (isNodeStream(stream)) {
      if (next !== null && (next?.closed || next?.destroyed)) {
        throw new ERR_STREAM_UNABLE_TO_PIPE();
      }

      if (end) {
        const { destroy, cleanup } = destroyer(stream, reading, writing);
        destroys.push(destroy);

        if (isReadable(stream) && isLastStream) {
          lastStreamCleanup.push(cleanup);
        }
      }

      // Catch stream errors that occur after pipe/pump has completed.
      function onError(err) {
        if (
          err &&
          err.name !== "AbortError" &&
          err.code !== "ERR_STREAM_PREMATURE_CLOSE" &&
          !(err.code === "ECONNRESET" &&
            streams.some((s) => isIncomingMessage(s)))
        ) {
          finishOnlyHandleError(err);
        }
      }
      stream.on("error", onError);
      // Simplified HTTP handling - remove extra close listeners for now

      // Special fix for HTTP IncomingMessage pipelines:
      // When a destination stream fails, make the source stream also emit an error
      // This ensures pipeline completion since regular pipelines complete on source errors
      if (i > 0 && isIncomingMessage(streams[0])) {
        const sourceStream = streams[0];
        stream.on("error", (err) => {
          // Propagate destination error to HTTP source to trigger pipeline completion
          process.nextTick(() => {
            sourceStream.emit("error", err);
            // Force immediate cleanup for HTTP streams to allow server.close() to complete
            if (!sourceStream.destroyed && sourceStream.destroy) {
              sourceStream.destroy(err);
            }
          });
        });
      }

      // Enhanced fix: also handle HTTP streams as destinations
      if (i > 0 && isServerResponse(stream)) {
        stream.on("error", (err) => {
          // When server response fails, ensure it's immediately destroyed
          process.nextTick(() => {
            if (!stream.destroyed && stream.destroy) {
              stream.destroy(err);
            }
          });
        });
      }

      // Special fix for ClientRequest destinations: when source is destroyed, properly end the request
      if (i > 0 && isClientRequest(stream)) {
        const sourceStream = streams[0];
        sourceStream.on("error", (err) => {
          // When source stream is destroyed/errored, end the ClientRequest properly
          process.nextTick(() => {
            if (!stream.destroyed && stream.end) {
              // End the request to signal EOF to server
              stream.end();
            }
          });
        });

        // Also handle the close event (destroy() emits close, not error)
        sourceStream.on("close", () => {
          // When source stream is closed/destroyed, end the ClientRequest properly
          process.nextTick(() => {
            if (!stream.destroyed && stream.end) {
              // End the request to signal EOF to server
              stream.end();
            }
          });
        });
      }
      if (isReadable(stream) && isLastStream) {
        lastStreamCleanup.push(() => {
          stream.removeListener("error", onError);
        });
      }
    }

    if (i === 0) {
      if (typeof stream === "function") {
        ret = stream({ signal });
        if (!isIterable(ret)) {
          throw new ERR_INVALID_RETURN_VALUE(
            "Iterable, AsyncIterable or Stream",
            "source",
            ret,
          );
        }
      } else if (
        isIterable(stream) || isReadableNodeStream(stream) ||
        isTransformStream(stream)
      ) {
        ret = stream;
      } else {
        ret = Duplex.from(stream);
      }
    } else if (typeof stream === "function") {
      if (isTransformStream(ret)) {
        ret = makeAsyncIterable(ret?.readable);
      } else {
        ret = makeAsyncIterable(ret);
      }
      ret = stream(ret, { signal });

      if (reading) {
        if (!isIterable(ret, true)) {
          throw new ERR_INVALID_RETURN_VALUE(
            "AsyncIterable",
            `transform[${i - 1}]`,
            ret,
          );
        }
      } else {
        PassThrough ??= _mod5;

        // If the last argument to pipeline is not a stream
        // we must create a proxy stream so that pipeline(...)
        // always returns a stream which can be further
        // composed through `.pipe(stream)`.

        const pt = new PassThrough({
          objectMode: true,
        });

        // Handle Promises/A+ spec, `then` could be a getter that throws on
        // second use.
        const then = ret?.then;
        if (typeof then === "function") {
          finishCount++;
          then.call(ret, (val) => {
            value = val;
            if (val != null) {
              pt.write(val);
            }
            if (end) {
              pt.end();
            }
            process.nextTick(finish);
          }, (err) => {
            pt.destroy(err);
            process.nextTick(finish, err);
          });
        } else if (isIterable(ret, true)) {
          finishCount++;
          pumpToNode(ret, pt, finish, { end });
        } else if (isReadableStream(ret) || isTransformStream(ret)) {
          const toRead = ret.readable || ret;
          finishCount++;
          pumpToNode(toRead, pt, finish, { end });
        } else {
          throw new ERR_INVALID_RETURN_VALUE(
            "AsyncIterable or Promise",
            "destination",
            ret,
          );
        }

        ret = pt;

        const { destroy, cleanup } = destroyer(ret, false, true);
        destroys.push(destroy);
        if (isLastStream) {
          lastStreamCleanup.push(cleanup);
        }
      }
    } else if (isNodeStream(stream)) {
      if (isReadableNodeStream(ret)) {
        // Use special HTTP-aware pumping for ClientRequest to handle buffering issues
        if (isClientRequest(stream)) {
          finishCount++;
          pumpToHTTPClientRequest(makeAsyncIterable(ret), stream, finish, {
            end,
          });
        } else {
          finishCount += 2;
          const cleanup = pipe(ret, stream, finish, finishOnlyHandleError, {
            end,
          });
          if (isReadable(stream) && isLastStream) {
            lastStreamCleanup.push(cleanup);
          }
        }
      } else if (isTransformStream(ret) || isReadableStream(ret)) {
        const toRead = ret.readable || ret;
        finishCount++;
        pumpToNode(toRead, stream, finish, { end });
      } else if (isIterable(ret)) {
        finishCount++;
        pumpToNode(ret, stream, finish, { end });
      } else {
        throw new ERR_INVALID_ARG_TYPE(
          "val",
          [
            "Readable",
            "Iterable",
            "AsyncIterable",
            "ReadableStream",
            "TransformStream",
          ],
          ret,
        );
      }
      ret = stream;
    } else if (isWebStream(stream)) {
      if (isReadableNodeStream(ret)) {
        finishCount++;
        pumpToWeb(makeAsyncIterable(ret), stream, finish, { end });
      } else if (isReadableStream(ret) || isIterable(ret)) {
        finishCount++;
        pumpToWeb(ret, stream, finish, { end });
      } else if (isTransformStream(ret)) {
        finishCount++;
        pumpToWeb(ret.readable, stream, finish, { end });
      } else {
        throw new ERR_INVALID_ARG_TYPE(
          "val",
          [
            "Readable",
            "Iterable",
            "AsyncIterable",
            "ReadableStream",
            "TransformStream",
          ],
          ret,
        );
      }
      ret = stream;
    } else {
      ret = Duplex.from(stream);
    }
  }

  if (signal?.aborted || outerSignal?.aborted) {
    process.nextTick(abort);
  }

  return ret;
}

function pipe(src, dst, finish, finishOnlyHandleError, { end }) {
  let ended = false;
  dst.on("close", () => {
    if (!ended) {
      // Finish if the destination closes before the source has completed.
      finishOnlyHandleError(new ERR_STREAM_PREMATURE_CLOSE());
    }
  });

  // Wrap pipe() to catch synchronous errors from _read (e.g. thrown in Readable._read).
  try {
    src.pipe(dst, { end: false });
  } catch (err) {
    finishOnlyHandleError(err);
    return;
  }
  if (end) {
    // Compat. Before node v10.12.0 stdio used to throw an error so
    // pipe() did/does not end() stdio destinations.
    // Now they allow it but "secretly" don't close the underlying fd.

    function endFn() {
      ended = true;
      dst.end();
    }

    if (isReadableFinished(src)) { // End the destination if the source has already ended.
      process.nextTick(endFn);
    } else {
      src.once("end", endFn);
    }
  } else {
    finish();
  }

  eos(src, { readable: true, writable: false }, (err) => {
    const rState = src._readableState;
    if (
      err &&
      err.code === "ERR_STREAM_PREMATURE_CLOSE" &&
      (rState?.ended && !rState.errored && !rState.errorEmitted)
    ) {
      // Some readable streams will emit 'close' before 'end'. However, since
      // this is on the readable side 'end' should still be emitted if the
      // stream has been ended and no error emitted. This should be allowed in
      // favor of backwards compatibility. Since the stream is piped to a
      // destination this should not result in any observable difference.
      // We don't need to check if this is a writable premature close since
      // eos will only fail with premature close on the reading side for
      // duplex streams.
      src
        .once("end", finish)
        .once("error", finish);
    } else {
      finish(err);
    }
  });
  return eos(dst, { readable: false, writable: true }, finish);
}

const _defaultExport2 = { pipelineImpl, pipeline };
export default _defaultExport2;
export { pipeline, pipelineImpl };
