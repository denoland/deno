// deno-lint-ignore-file
// Copyright 2018-2025 the Deno authors. MIT license.
import process from "node:process";
import { primordials } from "ext:core/mod.js";

import {
  isDuplexNodeStream,
  isIterable,
  isNodeStream,
  isReadable,
  isReadableNodeStream,
  isReadableStream,
  isWritable,
  isWritableNodeStream,
  isWritableStream,
} from "ext:deno_node/internal/streams/utils.js";

import eos from "ext:deno_node/internal/streams/end-of-stream.js";
import imported1 from "ext:deno_node/internal/errors.ts";
import { destroyer } from "ext:deno_node/internal/streams/destroy.js";
import Duplex from "ext:deno_node/internal/streams/duplex.js";
import Readable from "ext:deno_node/internal/streams/readable.js";
import Writable from "ext:deno_node/internal/streams/writable.js";
import from from "ext:deno_node/internal/streams/from.js";
import { isBlob } from "ext:deno_web/09_file.js";
import { AbortController } from "ext:deno_web/03_abort_signal.js";

const {
  AbortError,
  codes: {
    ERR_INVALID_ARG_TYPE,
    ERR_INVALID_RETURN_VALUE,
  },
} = imported1;

"use strict";

const {
  FunctionPrototypeCall,
  PromiseWithResolvers,
} = primordials;

let _Duplexify;

export default function duplexify(body, name) {
  // This is needed for pre node 17.
  class Duplexify extends Duplex {
    constructor(options) {
      super(options);

      // https://github.com/nodejs/node/pull/34385

      if (options?.readable === false) {
        this._readableState.readable = false;
        this._readableState.ended = true;
        this._readableState.endEmitted = true;
      }

      if (options?.writable === false) {
        this._writableState.writable = false;
        this._writableState.ending = true;
        this._writableState.ended = true;
        this._writableState.finished = true;
      }
    }
  }

  _Duplexify = Duplexify;

  if (isDuplexNodeStream(body)) {
    return body;
  }

  if (isReadableNodeStream(body)) {
    return _duplexify({ readable: body });
  }

  if (isWritableNodeStream(body)) {
    return _duplexify({ writable: body });
  }

  if (isNodeStream(body)) {
    return _duplexify({ writable: false, readable: false });
  }

  if (isReadableStream(body)) {
    return _duplexify({ readable: Readable.fromWeb(body) });
  }

  if (isWritableStream(body)) {
    return _duplexify({ writable: Writable.fromWeb(body) });
  }

  if (typeof body === "function") {
    const { value, write, final, destroy } = fromAsyncGen(body);

    // Body might be a constructor function instead of an async generator function.
    if (isDuplexNodeStream(value)) {
      return value;
    }

    if (isIterable(value)) {
      return from(Duplexify, value, {
        // TODO (ronag): highWaterMark?
        objectMode: true,
        write,
        final,
        destroy,
      });
    }

    const then = value?.then;
    if (typeof then === "function") {
      let d;

      const promise = FunctionPrototypeCall(
        then,
        value,
        (val) => {
          if (val != null) {
            throw new ERR_INVALID_RETURN_VALUE("nully", "body", val);
          }
        },
        (err) => {
          destroyer(d, err);
        },
      );

      return d = new Duplexify({
        // TODO (ronag): highWaterMark?
        objectMode: true,
        readable: false,
        write,
        final(cb) {
          final(async () => {
            try {
              await promise;
              process.nextTick(cb, null);
            } catch (err) {
              process.nextTick(cb, err);
            }
          });
        },
        destroy,
      });
    }

    throw new ERR_INVALID_RETURN_VALUE(
      "Iterable, AsyncIterable or AsyncFunction",
      name,
      value,
    );
  }

  if (isBlob(body)) {
    return duplexify(body.arrayBuffer());
  }

  if (isIterable(body)) {
    return from(Duplexify, body, {
      // TODO (ronag): highWaterMark?
      objectMode: true,
      writable: false,
    });
  }

  if (
    isReadableStream(body?.readable) &&
    isWritableStream(body?.writable)
  ) {
    return Duplexify.fromWeb(body);
  }

  if (
    typeof body?.writable === "object" ||
    typeof body?.readable === "object"
  ) {
    const readable = body?.readable
      ? isReadableNodeStream(body?.readable)
        ? body?.readable
        : duplexify(body.readable)
      : undefined;

    const writable = body?.writable
      ? isWritableNodeStream(body?.writable)
        ? body?.writable
        : duplexify(body.writable)
      : undefined;

    return _duplexify({ readable, writable });
  }

  const then = body?.then;
  if (typeof then === "function") {
    let d;

    FunctionPrototypeCall(
      then,
      body,
      (val) => {
        if (val != null) {
          d.push(val);
        }
        d.push(null);
      },
      (err) => {
        destroyer(d, err);
      },
    );

    return d = new Duplexify({
      objectMode: true,
      writable: false,
      read() {},
    });
  }

  throw new ERR_INVALID_ARG_TYPE(
    name,
    [
      "Blob",
      "ReadableStream",
      "WritableStream",
      "Stream",
      "Iterable",
      "AsyncIterable",
      "Function",
      "{ readable, writable } pair",
      "Promise",
    ],
    body,
  );
}

function fromAsyncGen(fn) {
  let { promise, resolve } = PromiseWithResolvers();
  const ac = new AbortController();
  const signal = ac.signal;
  const value = fn(
    async function* () {
      while (true) {
        const _promise = promise;
        promise = null;
        const { chunk, done, cb } = await _promise;
        process.nextTick(cb);
        if (done) return;
        if (signal.aborted) {
          throw new AbortError(undefined, { cause: signal.reason });
        }
        ({ promise, resolve } = PromiseWithResolvers());
        yield chunk;
      }
    }(),
    { signal },
  );

  return {
    value,
    write(chunk, encoding, cb) {
      const _resolve = resolve;
      resolve = null;
      _resolve({ chunk, done: false, cb });
    },
    final(cb) {
      const _resolve = resolve;
      resolve = null;
      _resolve({ done: true, cb });
    },
    destroy(err, cb) {
      ac.abort();
      cb(err);
    },
  };
}

function _duplexify(pair) {
  const r = pair.readable && typeof pair.readable.read !== "function"
    ? Readable.wrap(pair.readable)
    : pair.readable;
  const w = pair.writable;

  let readable = !!isReadable(r);
  let writable = !!isWritable(w);

  let ondrain;
  let onfinish;
  let onreadable;
  let onclose;
  let d;

  function onfinished(err) {
    const cb = onclose;
    onclose = null;

    if (cb) {
      cb(err);
    } else if (err) {
      d.destroy(err);
    }
  }

  // TODO(ronag): Avoid double buffering.
  // Implement Writable/Readable/Duplex traits.
  // See, https://github.com/nodejs/node/pull/33515.
  d = new _Duplexify({
    // TODO (ronag): highWaterMark?
    readableObjectMode: !!r?.readableObjectMode,
    writableObjectMode: !!w?.writableObjectMode,
    readable,
    writable,
  });

  if (writable) {
    eos(w, (err) => {
      writable = false;
      if (err) {
        destroyer(r, err);
      }
      onfinished(err);
    });

    d._write = function (chunk, encoding, callback) {
      if (w.write(chunk, encoding)) {
        callback();
      } else {
        ondrain = callback;
      }
    };

    d._final = function (callback) {
      w.end();
      onfinish = callback;
    };

    w.on("drain", function () {
      if (ondrain) {
        const cb = ondrain;
        ondrain = null;
        cb();
      }
    });

    w.on("finish", function () {
      if (onfinish) {
        const cb = onfinish;
        onfinish = null;
        cb();
      }
    });
  }

  if (readable) {
    eos(r, (err) => {
      readable = false;
      if (err) {
        destroyer(r, err);
      }
      onfinished(err);
    });

    r.on("readable", function () {
      if (onreadable) {
        const cb = onreadable;
        onreadable = null;
        cb();
      }
    });

    r.on("end", function () {
      d.push(null);
    });

    d._read = function () {
      while (true) {
        const buf = r.read();

        if (buf === null) {
          onreadable = d._read;
          return;
        }

        if (!d.push(buf)) {
          return;
        }
      }
    };
  }

  d._destroy = function (err, callback) {
    if (!err && onclose !== null) {
      err = new AbortError();
    }

    onreadable = null;
    ondrain = null;
    onfinish = null;

    if (onclose === null) {
      callback(err);
    } else {
      onclose = callback;
      destroyer(w, err);
      destroyer(r, err);
    }
  };

  return d;
}
