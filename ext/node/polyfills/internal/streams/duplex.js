// deno-lint-ignore-file
// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import _mod1 from "ext:deno_node/internal/streams/legacy.js";
import Readable from "ext:deno_node/internal/streams/readable.js";
import Writable from "ext:deno_node/internal/streams/writable.js";
import { addAbortSignal } from "ext:deno_node/internal/streams/add-abort-signal.js";
import destroyImpl from "ext:deno_node/internal/streams/destroy.js";
import { kOnConstructed } from "ext:deno_node/internal/streams/utils.js";
import * as _mod2 from "ext:deno_node/internal/webstreams/adapters.js";
import _mod3 from "ext:deno_node/internal/streams/duplexify.js";
const Stream = _mod1.Stream;
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

// a duplex stream is just a stream that is both readable and writable.
// Since JS doesn't have multiple prototype inheritance, this class
// prototypically inherits from Readable, and then parasitically from
// Writable.

"use strict";

const {
  ObjectDefineProperties,
  ObjectGetOwnPropertyDescriptor,
  ObjectKeys,
  ObjectSetPrototypeOf,
} = primordials;

ObjectSetPrototypeOf(Duplex.prototype, Readable.prototype);
ObjectSetPrototypeOf(Duplex, Readable);

{
  const keys = ObjectKeys(Writable.prototype);
  // Allow the keys array to be GC'ed.
  for (let i = 0; i < keys.length; i++) {
    const method = keys[i];
    Duplex.prototype[method] ||= Writable.prototype[method];
  }
}

// Use the `destroy` method of `Writable`.
Duplex.prototype.destroy = Writable.prototype.destroy;

function Duplex(options) {
  if (!(this instanceof Duplex)) {
    return new Duplex(options);
  }

  this._events ??= {
    close: undefined,
    error: undefined,
    prefinish: undefined,
    finish: undefined,
    drain: undefined,
    data: undefined,
    end: undefined,
    readable: undefined,
    // Skip uncommon events...
    // pause: undefined,
    // resume: undefined,
    // pipe: undefined,
    // unpipe: undefined,
    // [destroyImpl.kConstruct]: undefined,
    // [destroyImpl.kDestroy]: undefined,
  };

  this._readableState = new Readable.ReadableState(options, this, true);
  this._writableState = new Writable.WritableState(options, this, true);

  if (options) {
    this.allowHalfOpen = options.allowHalfOpen !== false;

    if (options.readable === false) {
      this._readableState.readable = false;
      this._readableState.ended = true;
      this._readableState.endEmitted = true;
    }

    if (options.writable === false) {
      this._writableState.writable = false;
      this._writableState.ending = true;
      this._writableState.ended = true;
      this._writableState.finished = true;
    }

    if (typeof options.read === "function") {
      this._read = options.read;
    }

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

    if (typeof options.construct === "function") {
      this._construct = options.construct;
    }

    if (options.signal) {
      addAbortSignal(options.signal, this);
    }
  } else {
    this.allowHalfOpen = true;
  }

  Stream.call(this, options);

  if (this._construct != null) {
    destroyImpl.construct(this, () => {
      this._readableState[kOnConstructed](this);
      this._writableState[kOnConstructed](this);
    });
  }
}

ObjectDefineProperties(Duplex.prototype, {
  writable: {
    __proto__: null,
    ...ObjectGetOwnPropertyDescriptor(Writable.prototype, "writable"),
  },
  writableHighWaterMark: {
    __proto__: null,
    ...ObjectGetOwnPropertyDescriptor(
      Writable.prototype,
      "writableHighWaterMark",
    ),
  },
  writableObjectMode: {
    __proto__: null,
    ...ObjectGetOwnPropertyDescriptor(Writable.prototype, "writableObjectMode"),
  },
  writableBuffer: {
    __proto__: null,
    ...ObjectGetOwnPropertyDescriptor(Writable.prototype, "writableBuffer"),
  },
  writableLength: {
    __proto__: null,
    ...ObjectGetOwnPropertyDescriptor(Writable.prototype, "writableLength"),
  },
  writableFinished: {
    __proto__: null,
    ...ObjectGetOwnPropertyDescriptor(Writable.prototype, "writableFinished"),
  },
  writableCorked: {
    __proto__: null,
    ...ObjectGetOwnPropertyDescriptor(Writable.prototype, "writableCorked"),
  },
  writableEnded: {
    __proto__: null,
    ...ObjectGetOwnPropertyDescriptor(Writable.prototype, "writableEnded"),
  },
  writableNeedDrain: {
    __proto__: null,
    ...ObjectGetOwnPropertyDescriptor(Writable.prototype, "writableNeedDrain"),
  },

  destroyed: {
    __proto__: null,
    get() {
      if (
        this._readableState === undefined ||
        this._writableState === undefined
      ) {
        return false;
      }
      return this._readableState.destroyed && this._writableState.destroyed;
    },
    set(value) {
      // Backward compatibility, the user is explicitly
      // managing destroyed.
      if (this._readableState && this._writableState) {
        this._readableState.destroyed = value;
        this._writableState.destroyed = value;
      }
    },
  },
});

let webStreamsAdapters;

// Lazy to avoid circular references
function lazyWebStreams() {
  if (webStreamsAdapters === undefined) {
    webStreamsAdapters = _mod2;
  }
  return webStreamsAdapters;
}

Duplex.fromWeb = function (pair, options) {
  return lazyWebStreams().newStreamDuplexFromReadableWritablePair(
    pair,
    options,
  );
};

Duplex.toWeb = function (duplex) {
  return lazyWebStreams().newReadableWritablePairFromDuplex(duplex);
};

let duplexify;

Duplex.from = function (body) {
  duplexify ??= _mod3;
  return duplexify(body, "body");
};
export default Duplex;
export { Duplex };
