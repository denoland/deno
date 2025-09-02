// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.

"use strict";

import { primordials } from "ext:core/mod.js";
const {
  Symbol,
} = primordials;

import { setImmediate } from "node:timers";
import process from "node:process";
import assert from "ext:deno_node/internal/assert.mjs";
import { Socket } from "node:net";
import { Buffer } from "node:buffer";
import { JSStream } from "ext:deno_node/internal_binding/js_stream.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import { ERR_STREAM_WRAP } from "ext:deno_node/internal/errors.ts";
import { ownerSymbol } from "ext:deno_node/internal/async_hooks.ts";

const kCurrentWriteRequest = Symbol("kCurrentWriteRequest");
const kCurrentShutdownRequest = Symbol("kCurrentShutdownRequest");
const kPendingShutdownRequest = Symbol("kPendingShutdownRequest");
const kPendingClose = Symbol("kPendingClose");

function isClosing() {
  return this[ownerSymbol].isClosing();
}

function onreadstart() {
  return this[ownerSymbol].readStart();
}

function onreadstop() {
  return this[ownerSymbol].readStop();
}

function onshutdown(req) {
  return this[ownerSymbol].doShutdown(req);
}

function onwrite(req, bufs) {
  return this[ownerSymbol].doWrite(req, bufs);
}

class JSStreamSocket extends Socket {
  constructor(stream) {
    const handle = new JSStream();
    handle.close = (cb) => {
      this.doClose(cb);
    };

    handle.isClosing = isClosing;
    handle.onreadstart = onreadstart;
    handle.onreadstop = onreadstop;
    handle.onshutdown = onshutdown;
    handle.onwrite = onwrite;

    // Add the missing handle methods that net.Socket expects
    handle.readStart = () => this.readStart();
    handle.readStop = () => this.readStop();
    handle.shutdown = (req) => this.doShutdown(req);
    handle.writeBuffer = (req, data) => this.doWrite(req, [data]);
    handle.writeUtf8String = (req, data) =>
      this.doWrite(req, [Buffer.from(data, "utf8")]);
    handle.writeAsciiString = (req, data) =>
      this.doWrite(req, [Buffer.from(data, "ascii")]);
    handle.writeLatin1String = (req, data) =>
      this.doWrite(req, [Buffer.from(data, "latin1")]);
    handle.writev = (req, chunks) => this.doWrite(req, chunks);
    handle.ref = () => {};
    handle.unref = () => {};

    stream.pause();
    stream.on("error", (err) => this.emit("error", err));

    const ondata = (chunk) => {
      if (
        typeof chunk === "string" ||
        stream.readableObjectMode === true
      ) {
        stream.pause();
        stream.removeListener("data", ondata);

        this.emit("error", new ERR_STREAM_WRAP());
        return;
      }

      if (this._handle) {
        this._handle.readBuffer(chunk);
      }
    };

    stream.on("data", ondata);

    stream.once("end", () => {
      if (this._handle) {
        this._handle.emitEOF();
      }
    });

    stream.once("close", () => {
      this.destroy();
    });

    super({ handle, manualStart: true });
    this.stream = stream;
    this[kCurrentWriteRequest] = null;
    this[kCurrentShutdownRequest] = null;
    this[kPendingShutdownRequest] = null;
    this[kPendingClose] = false;
    this.readable = stream.readable;
    this.writable = stream.writable;

    this.read(0);
  }

  static get StreamWrap() {
    return JSStreamSocket;
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

    assert(this[kCurrentWriteRequest] === null);
    assert(this[kCurrentShutdownRequest] === null);
    this[kCurrentShutdownRequest] = req;

    if (this[kPendingClose]) {
      return 0;
    }

    const handle = this._handle;
    assert(handle !== null);

    process.nextTick(() => {
      this.stream.end(() => {
        this.finishShutdown(handle, 0);
      });
    });
    return 0;
  }

  finishShutdown(handle, errCode) {
    if (this[kCurrentShutdownRequest] === null) {
      return;
    }
    const req = this[kCurrentShutdownRequest];
    this[kCurrentShutdownRequest] = null;
    handle.finishShutdown(req, errCode);
  }

  doWrite(req, bufs) {
    assert(this[kCurrentWriteRequest] === null);
    assert(this[kCurrentShutdownRequest] === null);

    if (this[kPendingClose]) {
      this[kCurrentWriteRequest] = req;
      return 0;
    } else if (this._handle === null) {
      return 0;
    }

    const handle = this._handle;
    let pending = bufs.length;

    const done = (err) => {
      if (!err && --pending !== 0) {
        return;
      }

      pending = 0;

      let errCode = 0;
      if (err) {
        errCode = codeMap.get("UV_EPIPE") || -32;
      }

      setImmediate(() => {
        this.finishWrite(handle, errCode);
      });
    };

    this.stream.cork();
    for (let i = 0; i < bufs.length; ++i) {
      this.stream.write(bufs[i], done);
    }
    this.stream.uncork();

    this[kCurrentWriteRequest] = req;

    return 0;
  }

  finishWrite(handle, errCode) {
    if (this[kCurrentWriteRequest] === null) {
      return;
    }
    const req = this[kCurrentWriteRequest];
    this[kCurrentWriteRequest] = null;

    handle.finishWrite(req, errCode);
    if (this[kPendingShutdownRequest]) {
      const req = this[kPendingShutdownRequest];
      this[kPendingShutdownRequest] = null;
      this.doShutdown(req);
    }
  }

  doClose(cb) {
    this[kPendingClose] = true;

    const handle = this._handle;

    this.stream.destroy();

    setImmediate(() => {
      // Should be already set by net.js, but don't assert since Deno might have different behavior
      // assert(this._handle === null);

      this.finishWrite(handle, codeMap.get("UV_ECANCELED") || -125);
      this.finishShutdown(handle, codeMap.get("UV_ECANCELED") || -125);

      this[kPendingClose] = false;

      if (cb) cb();
    });
  }
}

export default JSStreamSocket;
