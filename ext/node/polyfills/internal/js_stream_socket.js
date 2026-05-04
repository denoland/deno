// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
//
// Ported from Node.js lib/internal/js_stream_socket.js

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { core } from "ext:core/mod.js";
import { Socket } from "node:net";
const { nextTick } = core.loadExtScript("ext:deno_node/_next_tick.ts");
const { codeMap, UV_ECANCELED } = core.loadExtScript(
  "ext:deno_node/internal_binding/uv.ts",
);
import { setImmediate } from "node:timers";
const {
  kBytesWritten,
  kLastWriteWasAsync,
  streamBaseState,
} = core.loadExtScript("ext:deno_node/internal_binding/stream_wrap.ts");
import { Buffer } from "node:buffer";

const kCurrentWriteRequest = Symbol("kCurrentWriteRequest");
const kCurrentShutdownRequest = Symbol("kCurrentShutdownRequest");
const kPendingShutdownRequest = Symbol("kPendingShutdownRequest");
const kPendingClose = Symbol("kPendingClose");

// Mark JS stream handles so TLSWrap can detect them.
// Use Symbol.for so tls_wrap.ts can access it without importing
// (avoids circular dependency).
const kJSStreamHandle = Symbol.for("kJSStreamHandle");

function isClosing() {
  return this[kOwner].isClosing();
}
function onreadstart() {
  return this[kOwner].readStart();
}
function onreadstop() {
  return this[kOwner].readStop();
}

const kOwner = Symbol.for("kJSStreamOwner");

/* This class serves as a wrapper for when the Rust TLS layer wants access
 * to a standard JS stream. For example, TLS or HTTP2 do not operate on
 * network resources conceptually, although that is the common case; in
 * theory, they are completely composable and can work with any stream
 * resource they see.
 *
 * For the common case, i.e. a TLS socket wrapping around a net.Socket, we
 * can skip going through the JS layer and let TLS access the raw native
 * handle of a net.Socket. The flipside of this is that, to maintain
 * composability, we need a way to create "fake" net.Socket instances that
 * call back into a "real" JavaScript stream. JSStreamSocket is exactly this.
 */
class JSStreamSocket extends Socket {
  constructor(stream) {
    // Create a lightweight handle object that mimics what Node's
    // JSStream C++ binding provides. TLSWrap detects kJSStreamHandle
    // and uses attachJsStream() instead of attach().
    const handle = {
      [kJSStreamHandle]: true,
      [kOwner]: null, // set below
      close(cb) {
        handle[kOwner].doClose(cb);
      },
      isClosing,
      // Callbacks invoked by the owner (JSStreamSocket) methods
      onreadstart,
      onreadstop,
      // Methods called by net.Socket on the handle - delegate to owner
      readStart() {
        return handle[kOwner].readStart();
      },
      readStop() {
        return handle[kOwner].readStop();
      },
      // Write methods - delegate to owner.doWrite which writes to the
      // underlying stream and triggers req.oncomplete via finishWrite.
      writeBuffer(req, data) {
        const len = data.byteLength ?? data.length ?? 0;
        streamBaseState[kBytesWritten] = len;
        streamBaseState[kLastWriteWasAsync] = 1;
        return handle[kOwner].doWrite(req, [data]);
      },
      writev(req, chunks, allBuffers) {
        let bufs;
        let total = 0;
        if (allBuffers) {
          bufs = chunks;
          for (let i = 0; i < bufs.length; i++) {
            total += bufs[i].byteLength ?? bufs[i].length ?? 0;
          }
        } else {
          bufs = new Array(chunks.length >> 1);
          for (let i = 0; i < chunks.length; i += 2) {
            const chunk = chunks[i];
            const enc = chunks[i + 1];
            const buf = typeof chunk === "string"
              ? Buffer.from(chunk, enc)
              : chunk;
            bufs[i >> 1] = buf;
            total += buf.byteLength ?? buf.length ?? 0;
          }
        }
        streamBaseState[kBytesWritten] = total;
        streamBaseState[kLastWriteWasAsync] = 1;
        return handle[kOwner].doWrite(req, bufs);
      },
      writeAsciiString(req, data) {
        return this.writeBuffer(req, Buffer.from(data, "ascii"));
      },
      writeUtf8String(req, data) {
        return this.writeBuffer(req, Buffer.from(data, "utf8"));
      },
      writeLatin1String(req, data) {
        return this.writeBuffer(req, Buffer.from(data, "latin1"));
      },
      writeUcs2String(req, data) {
        return this.writeBuffer(req, Buffer.from(data, "utf16le"));
      },
      shutdown(req) {
        return handle[kOwner].doShutdown(req);
      },
      // These are set by TLSWrap after attachJsStream()
      readBuffer: null,
      emitEOF: null,
      reading: false,
    };

    stream.pause();
    stream.on("error", (err) => this.emit("error", err));
    const ondata = (chunk) => {
      if (
        typeof chunk === "string" ||
        stream.readableObjectMode === true
      ) {
        // Make sure that no further `data` events will happen.
        stream.pause();
        stream.removeListener("data", ondata);
        this.emit("error", new Error("Stream is not in binary mode"));
        return;
      }

      if (this._handle && this._handle.readBuffer) {
        this._handle.readBuffer(chunk);
      }
    };
    stream.on("data", ondata);
    stream.once("end", () => {
      if (this._handle && this._handle.emitEOF) {
        this._handle.emitEOF();
      }
    });
    // Some `Stream` don't pass `hasError` parameters when closed.
    stream.once("close", () => {
      this.destroy();
    });

    super({ handle, manualStart: true });
    handle[kOwner] = this;
    this.stream = stream;
    this[kCurrentWriteRequest] = null;
    this[kCurrentShutdownRequest] = null;
    this[kPendingShutdownRequest] = null;
    this[kPendingClose] = false;
    this.readable = stream.readable;
    this.writable = stream.writable;

    // Start reading.
    this.read(0);
  }

  isClosing() {
    return !this.readable || !this.writable;
  }

  readStart() {
    this.stream.resume();
    return 0;
  }

  readStop() {
    this.stream.pause();
    return 0;
  }

  doShutdown(req) {
    if (this[kCurrentWriteRequest] !== null) {
      this[kPendingShutdownRequest] = req;
      return 0;
    }

    this[kCurrentShutdownRequest] = req;

    if (this[kPendingClose]) {
      return 0;
    }

    const handle = this._handle;

    nextTick(() => {
      this.stream.end(() => {
        this.finishShutdown(handle, 0);
      });
    });
    return 0;
  }

  finishShutdown(_handle, errCode) {
    const req = this[kCurrentShutdownRequest];
    if (req === null) return;
    this[kCurrentShutdownRequest] = null;
    if (typeof req.oncomplete === "function") {
      req.oncomplete(errCode | 0);
    }
  }

  doWrite(req, bufs) {
    if (this[kPendingClose]) {
      this[kCurrentWriteRequest] = req;
      return 0;
    } else if (this._handle === null) {
      return 0;
    }

    const handle = this._handle;
    // deno-lint-ignore no-this-alias
    const self = this;

    let pending = bufs.length;

    this.stream.cork();
    for (let i = 0; i < bufs.length; ++i) {
      this.stream.write(bufs[i], done);
    }
    this.stream.uncork();

    this[kCurrentWriteRequest] = req;

    function done(err) {
      if (!err && --pending !== 0) return;
      pending = 0;

      let errCode = 0;
      if (err) {
        errCode = codeMap.get(err.code) || codeMap.get("EPIPE");
      }

      setImmediate(() => {
        self.finishWrite(handle, errCode);
      });
    }

    return 0;
  }

  finishWrite(_handle, errCode) {
    const req = this[kCurrentWriteRequest];
    if (req === null) return;
    this[kCurrentWriteRequest] = null;
    if (typeof req.oncomplete === "function") {
      req.oncomplete(errCode | 0);
    }

    if (this[kPendingShutdownRequest]) {
      const sreq = this[kPendingShutdownRequest];
      this[kPendingShutdownRequest] = null;
      this.doShutdown(sreq);
    }
  }

  doClose(cb) {
    this[kPendingClose] = true;
    const handle = this._handle;

    this.stream.destroy();

    setImmediate(() => {
      this.finishWrite(handle, UV_ECANCELED);
      this.finishShutdown(handle, UV_ECANCELED);
      this[kPendingClose] = false;
      if (cb) cb();
    });
  }
}

export { JSStreamSocket, kJSStreamHandle, kOwner };
export default JSStreamSocket;
