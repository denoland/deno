// Copyright Node.js contributors. All rights reserved. MIT License.
import { once } from "../_utils.ts";
import type Readable from "./readable.ts";
import type Stream from "./stream.ts";
import type { ReadableState } from "./readable.ts";
import type Writable from "./writable.ts";
import type { WritableState } from "./writable.ts";
import {
  ERR_INVALID_ARG_TYPE,
  ERR_STREAM_PREMATURE_CLOSE,
  NodeErrorAbstraction,
} from "../_errors.ts";

type StreamImplementations = Readable | Stream | Writable;

// TODO(Soremwar)
// Bring back once requests are implemented
// function isRequest(stream: Stream) {
//   return stream.setHeader && typeof stream.abort === "function";
// }

// deno-lint-ignore no-explicit-any
function isReadable(stream: any) {
  return typeof stream.readable === "boolean" ||
    typeof stream.readableEnded === "boolean" ||
    !!stream._readableState;
}

// deno-lint-ignore no-explicit-any
function isWritable(stream: any) {
  return typeof stream.writable === "boolean" ||
    typeof stream.writableEnded === "boolean" ||
    !!stream._writableState;
}

function isWritableFinished(stream: Writable) {
  if (stream.writableFinished) return true;
  const wState = stream._writableState;
  if (!wState || wState.errored) return false;
  return wState.finished || (wState.ended && wState.length === 0);
}

function nop() {}

function isReadableEnded(stream: Readable) {
  if (stream.readableEnded) return true;
  const rState = stream._readableState;
  if (!rState || rState.errored) return false;
  return rState.endEmitted || (rState.ended && rState.length === 0);
}

interface FinishedOptions {
  error?: boolean;
  readable?: boolean;
  writable?: boolean;
}

/**
 * Appends an ending callback triggered when a stream is no longer readable,
 * writable or has experienced an error or a premature close event
*/
export default function eos(
  stream: StreamImplementations,
  options: FinishedOptions | null,
  callback: (err?: NodeErrorAbstraction | null) => void,
): () => void;
export default function eos(
  stream: StreamImplementations,
  callback: (err?: NodeErrorAbstraction | null) => void,
): () => void;
export default function eos(
  stream: StreamImplementations,
  x: FinishedOptions | ((err?: NodeErrorAbstraction | null) => void) | null,
  y?: (err?: NodeErrorAbstraction | null) => void,
) {
  let opts: FinishedOptions;
  let callback: (err?: NodeErrorAbstraction | null) => void;

  if (!y) {
    if (typeof x !== "function") {
      throw new ERR_INVALID_ARG_TYPE("callback", "function", x);
    }
    opts = {};
    callback = x;
  } else {
    if (!x || Array.isArray(x) || typeof x !== "object") {
      throw new ERR_INVALID_ARG_TYPE("opts", "object", x);
    }
    opts = x;

    if (typeof y !== "function") {
      throw new ERR_INVALID_ARG_TYPE("callback", "function", y);
    }
    callback = y;
  }

  callback = once(callback);

  const readable = opts.readable ?? isReadable(stream);
  const writable = opts.writable ?? isWritable(stream);

  // deno-lint-ignore no-explicit-any
  const wState: WritableState | undefined = (stream as any)._writableState;
  // deno-lint-ignore no-explicit-any
  const rState: ReadableState | undefined = (stream as any)._readableState;
  const validState = wState || rState;

  const onlegacyfinish = () => {
    if (!(stream as Writable).writable) {
      onfinish();
    }
  };

  let willEmitClose = (
    validState?.autoDestroy &&
    validState?.emitClose &&
    validState?.closed === false &&
    isReadable(stream) === readable &&
    isWritable(stream) === writable
  );

  let writableFinished = (stream as Writable).writableFinished ||
    wState?.finished;
  const onfinish = () => {
    writableFinished = true;
    // deno-lint-ignore no-explicit-any
    if ((stream as any).destroyed) {
      willEmitClose = false;
    }

    if (willEmitClose && (!(stream as Readable).readable || readable)) {
      return;
    }
    if (!readable || readableEnded) {
      callback.call(stream);
    }
  };

  let readableEnded = (stream as Readable).readableEnded || rState?.endEmitted;
  const onend = () => {
    readableEnded = true;
    // deno-lint-ignore no-explicit-any
    if ((stream as any).destroyed) {
      willEmitClose = false;
    }

    if (willEmitClose && (!(stream as Writable).writable || writable)) {
      return;
    }
    if (!writable || writableFinished) {
      callback.call(stream);
    }
  };

  const onerror = (err: NodeErrorAbstraction) => {
    callback.call(stream, err);
  };

  const onclose = () => {
    if (readable && !readableEnded) {
      if (!isReadableEnded(stream as Readable)) {
        return callback.call(stream, new ERR_STREAM_PREMATURE_CLOSE());
      }
    }
    if (writable && !writableFinished) {
      if (!isWritableFinished(stream as Writable)) {
        return callback.call(stream, new ERR_STREAM_PREMATURE_CLOSE());
      }
    }
    callback.call(stream);
  };

  // TODO(Soremwar)
  // Bring back once requests are implemented
  // const onrequest = () => {
  //   stream.req.on("finish", onfinish);
  // };

  // TODO(Soremwar)
  // Bring back once requests are implemented
  // if (isRequest(stream)) {
  //   stream.on("complete", onfinish);
  //   stream.on("abort", onclose);
  //   if (stream.req) {
  //     onrequest();
  //   } else {
  //     stream.on("request", onrequest);
  //   }
  // } else
  if (writable && !wState) {
    stream.on("end", onlegacyfinish);
    stream.on("close", onlegacyfinish);
  }

  // TODO(Soremwar)
  // Bring back once requests are implemented
  // if (typeof stream.aborted === "boolean") {
  //   stream.on("aborted", onclose);
  // }

  stream.on("end", onend);
  stream.on("finish", onfinish);
  if (opts.error !== false) stream.on("error", onerror);
  stream.on("close", onclose);

  const closed = (
    wState?.closed ||
    rState?.closed ||
    wState?.errorEmitted ||
    rState?.errorEmitted ||
    // TODO(Soremwar)
    // Bring back once requests are implemented
    // (rState && stream.req && stream.aborted) ||
    (
      (!writable || wState?.finished) &&
      (!readable || rState?.endEmitted)
    )
  );

  if (closed) {
    queueMicrotask(callback);
  }

  return function () {
    callback = nop;
    stream.removeListener("aborted", onclose);
    stream.removeListener("complete", onfinish);
    stream.removeListener("abort", onclose);
    // TODO(Soremwar)
    // Bring back once requests are implemented
    // stream.removeListener("request", onrequest);
    // if (stream.req) stream.req.removeListener("finish", onfinish);
    stream.removeListener("end", onlegacyfinish);
    stream.removeListener("close", onlegacyfinish);
    stream.removeListener("finish", onfinish);
    stream.removeListener("end", onend);
    stream.removeListener("error", onerror);
    stream.removeListener("close", onclose);
  };
}
