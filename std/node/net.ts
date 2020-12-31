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

'use strict';

import EventEmitter from "./events.ts";
import { Stream as stream } from "./stream.ts"

// TODO(ebebbington): Should this be in a separate file? eg `./_net/net.ts`
// internals/net
const normalizedArgsSymbol = Symbol("normalizedArgs")
const v4Seg = '(?:[0-9]|[1-9][0-9]|1[0-9][0-9]|2[0-4][0-9]|25[0-5])';
const v4Str = `(${v4Seg}[.]){3}${v4Seg}`;
const IPv4Reg = new RegExp(`^${v4Str}$`);
const v6Seg = '(?:[0-9a-fA-F]{1,4})';
const IPv6Reg = new RegExp('^(' +
  `(?:${v6Seg}:){7}(?:${v6Seg}|:)|` +
  `(?:${v6Seg}:){6}(?:${v4Str}|:${v6Seg}|:)|` +
  `(?:${v6Seg}:){5}(?::${v4Str}|(:${v6Seg}){1,2}|:)|` +
  `(?:${v6Seg}:){4}(?:(:${v6Seg}){0,1}:${v4Str}|(:${v6Seg}){1,3}|:)|` +
  `(?:${v6Seg}:){3}(?:(:${v6Seg}){0,2}:${v4Str}|(:${v6Seg}){1,4}|:)|` +
  `(?:${v6Seg}:){2}(?:(:${v6Seg}){0,3}:${v4Str}|(:${v6Seg}){1,5}|:)|` +
  `(?:${v6Seg}:){1}(?:(:${v6Seg}){0,4}:${v4Str}|(:${v6Seg}){1,6}|:)|` +
  `(?::((?::${v6Seg}){0,5}:${v4Str}|(?::${v6Seg}){1,7}|:))` +
  ')(%[0-9a-zA-Z-.:]{1,})?$');
function isIPv4(s: string): boolean {
  return IPv4Reg.test(s);
}
function isIPv6(s: string): boolean {
  return IPv6Reg.test(s);
}
function isIP(s: string): number {
  if (isIPv4(s)) return 4;
  if (isIPv6(s)) return 6;
  return 0;
}

import{ assert } from "../testing/asserts.ts"
const {
  UV_EADDRINUSE,
  UV_ENOTCONN
} = internalBinding('uv');

const { guessHandleType } = internalBinding('util');
const { ShutdownWrap } = internalBinding('stream_wrap');
const {
  TCP,
  TCPConnectWrap,
  constants: TCPConstants
} = internalBinding('tcp_wrap');
const {
  Pipe,
  PipeConnectWrap,
  constants: PipeConstants
} = internalBinding('pipe_wrap');

const {
  newAsyncId,
  defaultTriggerAsyncIdScope,
  symbols: { async_id_symbol, owner_symbol }
} = require('internal/async_hooks');
const {
  writevGeneric,
  writeGeneric,
  onStreamRead,
  kAfterAsyncWrite,
  kHandle,
  kUpdateTimer,
  setStreamTimeout,
  kBuffer,
  kBufferCb,
  kBufferGen
} = require('internal/stream_base_commons');
const {
  codes: {
    ERR_INVALID_ADDRESS_FAMILY,
    ERR_INVALID_ARG_TYPE,
    ERR_INVALID_FD_TYPE,
    ERR_INVALID_IP_ADDRESS,
    ERR_SOCKET_CLOSED,
    ERR_MISSING_ARGS,
  },
  errnoException,
  exceptionWithHostPort,
} = require('internal/errors');
const { isUint8Array } = require('internal/util/types');
const {
  validateInt32,
  validatePort,
  validateString
} = require('internal/validators');
const kLastWriteQueueSize = Symbol('lastWriteQueueSize');
const {
  DTRACE_NET_STREAM_END
} = require('internal/dtrace');

// Lazy loaded to improve startup performance.
let dns;

const { clearTimeout } = require('timers');
const { kTimeout } = require('internal/timers');

const DEFAULT_IPV4_ADDR = '0.0.0.0';
const DEFAULT_IPV6_ADDR = '::';

const isWindows = process.platform === 'win32';

const noop = Function.prototype;

function createHandle(fd, is_server: boolean) {
  validateInt32(fd, 'fd', 0);
  const type = guessHandleType(fd);
  if (type === 'PIPE') {
    return new Pipe(
      is_server ? PipeConstants.SERVER : PipeConstants.SOCKET
    );
  }

  if (type === 'TCP') {
    return new TCP(
      is_server ? TCPConstants.SERVER : TCPConstants.SOCKET
    );
  }

  throw new ERR_INVALID_FD_TYPE(type);
}

function getNewAsyncId(handle) {
  return (!handle || typeof handle.getAsyncId !== 'function') ?
    newAsyncId() : handle.getAsyncId();
}

function isPipeName(s) {
  return typeof s === 'string' && toNumber(s) === false;
}

// Returns an array [options, cb], where options is an object,
// cb is either a function or null.
// Used to normalize arguments of Socket.prototype.connect() and
// Server.prototype.listen(). Possible combinations of parameters:
//   (options[...][, cb])
//   (path[...][, cb])
//   ([port][, host][...][, cb])
// For Socket.prototype.connect(), the [...] part is ignored
// For Server.prototype.listen(), the [...] part is [, backlog]
// but will not be handled here (handled in listen())
function normalizeArgs(args) {
  let arr;

  if (args.length === 0) {
    arr = [{}, null];
    arr[normalizedArgsSymbol] = true;
    return arr;
  }

  const arg0 = args[0];
  let options = {};
  if (typeof arg0 === 'object' && arg0 !== null) {
    // (options[...][, cb])
    options = arg0;
  } else if (isPipeName(arg0)) {
    // (path[...][, cb])
    options.path = arg0;
  } else {
    // ([port][, host][...][, cb])
    options.port = arg0;
    if (args.length > 1 && typeof args[1] === 'string') {
      options.host = args[1];
    }
  }

  const cb = args[args.length - 1];
  if (typeof cb !== 'function')
    arr = [options, null];
  else
    arr = [options, cb];

  arr[normalizedArgsSymbol] = true;
  return arr;
}

// Called when creating new Socket, or when re-using a closed Socket
function initSocketHandle(self) {
  self._undestroy();
  self._sockname = null;

  // Handle creation may be deferred to bind() or connect() time.
  if (self._handle) {
    self._handle[owner_symbol] = self;
    self._handle.onread = onStreamRead;
    self[async_id_symbol] = getNewAsyncId(self._handle);

    let userBuf = self[kBuffer];
    if (userBuf) {
      const bufGen = self[kBufferGen];
      if (bufGen !== null) {
        userBuf = bufGen();
        if (!isUint8Array(userBuf))
          return;
        self[kBuffer] = userBuf;
      }
      self._handle.useUserBuffer(userBuf);
    }
  }
}

const kBytesRead = Symbol('kBytesRead');
const kBytesWritten = Symbol('kBytesWritten');
const kSetNoDelay = Symbol('kSetNoDelay');

interface SocketOptions {
  // user props
  handle?: any
  fd?: any
  onread?: any
  readable?: any
  pauseOnCreate?: boolean
  manualStart?: boolean

  // props assigned in constructor
  allowHalfOpen?: boolean,
  emitClose?: boolean,
  autoDestroy?: boolean,
  decodeStrings?: boolean
}

export class Socket {
  public connecting: boolean
  public [async_id_symbol]: number
  private _hadError: boolean
  private _handle: any
  public [kHandle]: any
  private _parent: any
  private _host: any
  public [kSetNoDelay]: boolean
  public [kLastWriteQueueSize]: number
  public [kTimeout]: any
  public [kBuffer]: any
  public [kBufferCb]: any
  public [kBufferGen]: any
  private _pendingData: any
  private _pendingEncoding; any
  public readableFlowing: boolean
  public server: any
  public _server: any
  public pending: boolean
  public readable: any
  public writable: any
  public writableFinished: boolean
  public destroyed: boolean
  public _sockname: null | any
  constructor(options: SocketOptions) {
    super(options)
    this.connecting = false;
    // Problem with this is that users can supply their own handle, that may not
    // have _handle.getAsyncId(). In this case an[async_id_symbol] should
    // probably be supplied by async_hooks.
    this[async_id_symbol] = -1;
    this._hadError = false;
    this[kHandle] = null;
    this._parent = null;
    this._host = null;
    this[kSetNoDelay] = false;
    this[kLastWriteQueueSize] = 0;
    this[kTimeout] = null;
    this[kBuffer] = null;
    this[kBufferCb] = null;
    this[kBufferGen] = null;

    // Default to *not* allowing half open sockets.
    options.allowHalfOpen = Boolean(options.allowHalfOpen);
    // For backwards compat do not emit close on destroy.
    options.emitClose = false;
    options.autoDestroy = true;
    // Handle strings directly.
    options.decodeStrings = false;
    Reflect(stream.Duplex, this, [options]);

    if (options.handle) {
      this._handle = options.handle; // private
      this[async_id_symbol] = getNewAsyncId(this._handle);
    } else
      if (options.fd !== undefined) {
      const {fd} = options;
      let err;

      // createHandle will throw ERR_INVALID_FD_TYPE if `fd` is not
      // a valid `PIPE` or `TCP` descriptor
      this._handle = createHandle(fd, false);

      err = this._handle.open(fd);

      // While difficult to fabricate, in some architectures
      // `open` may return an error code for valid file descriptors
      // which cannot be opened. This is difficult to test as most
      // un-openable fds will throw on `createHandle`
      if (err)
        throw errnoException(err, 'open');

      this[async_id_symbol] = this._handle.getAsyncId();

      if ((fd === 1 || fd === 2) &&
        (this._handle instanceof Pipe) && isWindows) {
        // Make stdout and stderr blocking on Windows
        err = this._handle.setBlocking(true);
        if (err)
          throw errnoException(err, 'setBlocking');

        // makeSyncWrite adjusts this value like the original handle would, so
        // we need to let it do that by turning it into a writable, own
        // property.
        Object.defineProperty(this._handle, 'bytesWritten', {
          value: 0, writable: true
        });
      }
    }

    const onread = options.onread;
    if (onread !== null && typeof onread === 'object' &&
      (isUint8Array(onread.buffer) || typeof onread.buffer === 'function') &&
      typeof onread.callback === 'function') {
      if (typeof onread.buffer === 'function') {
        this[kBuffer] = true;
        this[kBufferGen] = onread.buffer;
      } else {
        this[kBuffer] = onread.buffer;
      }
      this[kBufferCb] = onread.callback;
    }

    // Shut down the socket when we're finished with it.
    this.on('end', onReadableStreamEnd);

    initSocketHandle(this);

    this._pendingData = null;
    this._pendingEncoding = '';

    // If we have a handle, then start the flow of data into the
    // buffer.  if not, then this will happen when we connect
    if (this._handle && options.readable !== false) {
      if (options.pauseOnCreate) {
        // Stop the handle from reading and pause the stream
        this._handle.reading = false;
        this._handle.readStop();
        this.readableFlowing = false;
      } else if (!options.manualStart) {
        this.read(0);
      }
    }

    // Reserve properties
    this.server = null;
    this._server = null;

    // Used after `.destroy()`
    this[kBytesRead] = 0;
    this[kBytesWritten] = 0;
  }

  // Refresh existing timeouts.
  private _unrefTimer () {
    for (let s = this; s !== null; s = s._parent) {
      if (s[kTimeout])
        s[kTimeout].refresh();
    }
  }

  // Socket.prototype.setTimeout = setStreamTimeout;
  public setTimeout () {
    setStreamTimeout()
  }

  public setKeepAlive (setting: any, msecs: number) {
    if (!this._handle) {
      this.once('connect', () => this.setKeepAlive(setting, msecs));
      return this;
    }

    if (this._handle.setKeepAlive)
      this._handle.setKeepAlive(setting, ~~(msecs / 1000));

    return this;
  }

  public setNoDelay (enable: boolean) {
    if (!this._handle) {
      this.once('connect',
        enable ? this.setNoDelay : () => this.setNoDelay(enable));
      return this;
    }

    // Backwards compatibility: assume true when `enable` is omitted
    const newValue = enable === undefined ? true : !!enable;
    if (this._handle.setNoDelay && newValue !== this[kSetNoDelay]) {
      this[kSetNoDelay] = newValue;
      this._handle.setNoDelay(newValue);
    }

    return this;
  }

  public address () {
    return this._getsockname();
  }

  public get pending () {
    return !this._handle || this.connecting
  }

  public get readyState () {
    if (this.connecting) {
      return 'opening';
    } else if (this.readable && this.writable) {
      return 'open';
    } else if (this.readable && !this.writable) {
      return 'readOnly';
    } else if (!this.readable && this.writable) {
      return 'writeOnly';
    }
    return 'closed';
  }

  public get bufferSize () {
    if (this._handle) {
      return this.writableLength;
    }
  }

  public get [kUpdateTimer] () {
    return this._unrefTimer;
  }

  public end (data, encoding, callback) {
    Reflect(stream.Duplex.prototype.end, this, [data, encoding, callback]);
    DTRACE_NET_STREAM_END(this);
    return this;
  }

  public pause () {
    if (this[kBuffer] && !this.connecting && this._handle &&
      this._handle.reading) {
      this._handle.reading = false;
      if (!this.destroyed) {
        const err = this._handle.readStop();
        if (err)
          this.destroy(errnoException(err, 'read')); // TODO(ebebbington): Maybe `destroy()` is from duplex?
      }
    }
    return Function.prototype.call(stream.Duplex.prototype.pause, this);
  }

  public resume () {
    if (this[kBuffer] && !this.connecting && this._handle &&
      !this._handle.reading) {
      tryReadStart(this);
    }
    return Function.prototype.call(stream.Duplex.prototype.resume, this);
  }

  public read (n) {
    if (this[kBuffer] && !this.connecting && this._handle &&
      !this._handle.reading) {
      tryReadStart(this);
    }
    return Reflect(stream.Duplex.prototype.read, this, [n]);
  }

  public destroySoon () {
    if (this.writable)
      this.end();

    if (this.writableFinished)
      this.destroy();
    else
      this.once('finish', this.destroy);
  }

  public [kAfterAsyncWrite] () {
    this[kLastWriteQueueSize] = 0;
  }

  // We allow any here because the code within shows us that `args` can just  be way too generic to typee properly, and be readable
  public connect (...args: any[]) {
    let normalized;
    // If passed an array, it's treated as an array of arguments that have
    // already been normalized (so we don't normalize more than once). This has
    // been solved before in https://github.com/nodejs/node/pull/12342, but was
    // reverted as it had unintended side effects.
    if (Array.isArray(args[0]) && args[0][normalizedArgsSymbol]) {
      normalized = args[0];
    } else {
      normalized = normalizeArgs(args);
    }
    const options = normalized[0];
    const cb = normalized[1];

    // options.port === null will be checked later.
    if (options.port === undefined && options.path == null)
      throw new ERR_MISSING_ARGS(['options', 'port', 'path']);

    if (this.write !== Socket.prototype.write)
      this.write = Socket.prototype.write;

    if (this.destroyed) {
      this._handle = null;
      this._peername = null;
      this._sockname = null;
    }

    const { path } = options;
    const pipe = !!path;

    if (!this._handle) {
      this._handle = pipe ?
        new Pipe(PipeConstants.SOCKET) :
        new TCP(TCPConstants.SOCKET);
      initSocketHandle(this);
    }

    if (cb !== null) {
      this.once('connect', cb);
    }

    this._unrefTimer();

    this.connecting = true;

    if (pipe) {
      validateString(path, 'options.path');
      defaultTriggerAsyncIdScope(
        this[async_id_symbol], internalConnect, this, path
      );
    } else {
      lookupAndConnect(this, options);
    }
    return this;
  }

  public ref () {
    if (!this._handle) {
      this.once('connect', this.ref);
      return this;
    }

    if (typeof this._handle.ref === 'function') {
      this._handle.ref();
    }

    return this;
  }

  public unref () {
    if (!this._handle) {
      this.once('connect', this.unref);
      return this;
    }

    if (typeof this._handle.unref === 'function') {
      this._handle.unref();
    }

    return this;
  }

  private _onTimeout () {
    const handle = this._handle;
    const lastWriteQueueSize = this[kLastWriteQueueSize];
    if (lastWriteQueueSize > 0 && handle) {
      // `lastWriteQueueSize !== writeQueueSize` means there is
      // an active write in progress, so we suppress the timeout.
      const { writeQueueSize } = handle;
      if (lastWriteQueueSize !== writeQueueSize) {
        this[kLastWriteQueueSize] = writeQueueSize;
        this._unrefTimer();
        return;
      }
    }
    this.emit('timeout');
  }

  private get _connecting () {
    return this.connecting
  }

  private _destroy (exception, cb) {
    this.connecting = false;

    for (let s = this; s !== null; s = s._parent) {
      clearTimeout(s[kTimeout]);
    }

    if (this._handle) {
      const isException = exception ? true : false;
      // `bytesRead` and `kBytesWritten` should be accessible after `.destroy()`
      this[kBytesRead] = this._handle.bytesRead;
      this[kBytesWritten] = this._handle.bytesWritten;

      this._handle.close(() => {
        this.emit('close', isException);
      });
      this._handle.onread = noop;
      this._handle = null;
      this._sockname = null;
      cb(exception);
    } else {
      cb(exception);
      process.nextTick(emitCloseNT, this);
    }

    if (this._server) {
      this._server._connections--;
      if (this._server._emitCloseIfDrained) {
        this._server._emitCloseIfDrained();
      }
    }
  }

  // Just call handle.readStart until we have enough in the buffer
  private _read (n) {
    if (this.connecting || !this._handle) {
      this.once('connect', () => this._read(n));
    } else if (!this._handle.reading) {
      tryReadStart(this);
    }
  }

  // The user has called .end(), and all the bytes have been
  // sent out to the other side.
  private _final (cb: (err?: Error) => any) {
    // If still connecting - defer handling `_final` until 'connect' will happen
    if (this.pending) {
      return this.once('connect', () => this._final(cb));
    }

    if (!this._handle)
      return cb();

    const req = new ShutdownWrap();
    req.oncomplete = afterShutdown;
    req.handle = this._handle;
    req.callback = cb;
    const err = this._handle.shutdown(req);

    if (err === 1 || err === UV_ENOTCONN)  // synchronous finish
      return cb();
    else if (err !== 0)
      return cb(errnoException(err, 'shutdown'));
  }

  private _getpeername () {
    if (!this._handle || !this._handle.getpeername) {
      return this._peername || {};
    } else if (!this._peername) {
      this._peername = {};
      // FIXME(bnoordhuis) Throw when the return value is not 0?
      this._handle.getpeername(this._peername);
    }
    return this._peername;
  }

  private _getsockname () {
    if (!this._handle || !this._handle.getsockname) {
      return {};
    } else if (!this._sockname) {
      this._sockname = {};
      // FIXME(bnoordhuis) Throw when the return value is not 0? <-- From node, nothing to do with Deno
      this._handle.getsockname(this._sockname);
    }
    return this._sockname;
  }

  private _writeGeneric (writev: boolean, data, encoding, cb) {
    // If we are still connecting, then buffer this for later.
    // The Writable logic will buffer up any more writes while
    // waiting for this one to be done.
    if (this.connecting) {
      this._pendingData = data;
      this._pendingEncoding = encoding;
      this.once('connect', function connect() {
        this._writeGeneric(writev, data, encoding, cb);
      });
      return;
    }
    this._pendingData = null;
    this._pendingEncoding = '';

    if (!this._handle) {
      cb(new ERR_SOCKET_CLOSED());
      return false;
    }

    this._unrefTimer();

    let req;
    if (writev)
      req = writevGeneric(this, data, cb);
    else
      req = writeGeneric(this, data, encoding, cb);
    if (req.async)
      this[kLastWriteQueueSize] = req.bytes;
  }

  private _writev (chunks, cb) {
    this._writeGeneric(true, chunks, '', cb);
  }

  private _write (data, encoding, cb) {
    this._writeGeneric(false, data, encoding, cb);
  }

  private get _handle () {
    return this[kHandle]
  }

  private set _handle (v) {
    this[kHandle] = v
  }
}

function applyMixins(derivedConstructor: any, baseConstructors: any[]) {
  baseConstructors.forEach(baseConstructor => {
    Object.getOwnPropertyNames(baseConstructor.prototype)
      .forEach(name => {
        Object.defineProperty(derivedConstructor.prototype,
          name,
          Object.
          getOwnPropertyDescriptor(
            baseConstructor.prototype,
            name
          )
        );
      });
  });
}
applyMixins(Socket, [stream.Duplex, EventEmitter]) // to also make socket extend Duplex and EventEmitter
// ObjectSetPrototypeOf(Socket.prototype, stream.Duplex.prototype);
// ObjectSetPrototypeOf(Socket, stream.Duplex);

function afterShutdown() {
  this.callback();
}

// Provide a better error message when we call end() as a result
// of the other side sending a FIN.  The standard 'write after end'
// is overly vague, and makes it seem like the user's code is to blame.
function writeAfterFIN(chunk, encoding, cb) {
  if (!this.writableEnded) {
    return Reflect(
      stream.Duplex.prototype.write, this, [chunk, encoding, cb]);
  }

  if (typeof encoding === 'function') {
    cb = encoding;
    encoding = null;
  }

  // eslint-disable-next-line no-restricted-syntax
  const er = new Error('This socket has been ended by the other party');
  er.code = 'EPIPE';
  if (typeof cb === 'function') {
    defaultTriggerAsyncIdScope(this[async_id_symbol], process.nextTick, cb, er);
  }
  this.destroy(er);

  return false;
}

function tryReadStart(socket) {
  // Not already reading, start the flow
  socket._handle.reading = true;
  const err = socket._handle.readStart();
  if (err)
    socket.destroy(errnoException(err, 'read'));
}

// Called when the 'end' event is emitted.
function onReadableStreamEnd() {
  if (!this.allowHalfOpen) {
    this.write = writeAfterFIN;
  }
}

// Due to how this is written, the callback could be literally anything eg params, return value etc
function protoGetter(name: string, callback: any) {
  Object.defineProperty(Socket.prototype, name, {
    configurable: false,
    enumerable: true,
    get: callback
  });
}

protoGetter('bytesRead', function bytesRead() {
  return this._handle ? this._handle.bytesRead : this[kBytesRead];
});

protoGetter('remoteAddress', function remoteAddress() {
  return this._getpeername().address;
});

protoGetter('remoteFamily', function remoteFamily() {
  return this._getpeername().family;
});

protoGetter('remotePort', function remotePort() {
  return this._getpeername().port;
});

protoGetter('localAddress', function localAddress() {
  return this._getsockname().address;
});

protoGetter('localPort', function localPort() {
  return this._getsockname().port;
});

// Legacy alias. Having this is probably being overly cautious, but it doesn't
// really hurt anyone either. This can probably be removed safely if desired.
protoGetter('_bytesDispatched', function _bytesDispatched() {
  return this._handle ? this._handle.bytesWritten : this[kBytesWritten];
});

protoGetter('bytesWritten', function bytesWritten() {
  let bytes = this._bytesDispatched;
  const data = this._pendingData;
  const encoding = this._pendingEncoding;
  const writableBuffer = this.writableBuffer;

  if (!writableBuffer)
    return undefined;

  for (const el of writableBuffer) {
    bytes += el.chunk instanceof Buffer ?
      el.chunk.length :
      Buffer.byteLength(el.chunk, el.encoding);
  }

  if (Array.isArray(data)) {
    // Was a writev, iterate over chunks to get total length
    for (let i = 0; i < data.length; i++) {
      const chunk = data[i];

      if (data.allBuffers || chunk instanceof Buffer)
        bytes += chunk.length;
      else
        bytes += Buffer.byteLength(chunk.chunk, chunk.encoding);
    }
  } else if (data) {
    // Writes are either a string or a Buffer.
    if (typeof data !== 'string')
      bytes += data.length;
    else
      bytes += Buffer.byteLength(data, encoding);
  }

  return bytes;
});

function checkBindError(err, port, handle) {
  // EADDRINUSE may not be reported until we call listen() or connect().
  // To complicate matters, a failed bind() followed by listen() or connect()
  // will implicitly bind to a random port. Ergo, check that the socket is
  // bound to the expected port before calling listen() or connect().
  //
  // FIXME(bnoordhuis) Doesn't work for pipe handles, they don't have a
  // getsockname() method. Non-issue for now, the cluster module doesn't
  // really support pipes anyway.
  if (err === 0 && port > 0 && handle.getsockname) {
    const out = {};
    err = handle.getsockname(out);
    if (err === 0 && port !== out.port) {
      err = UV_EADDRINUSE;
    }
  }
  return err;
}

function internalConnect(
  self, address, port, addressType, localAddress, localPort, flags) {
  // TODO return promise from Socket.prototype.connect which
  // wraps _connectReq.

  assert(self.connecting);

  let err;

  if (localAddress || localPort) {
    if (addressType === 4) {
      localAddress = localAddress || DEFAULT_IPV4_ADDR;
      err = self._handle.bind(localAddress, localPort);
    } else { // addressType === 6
      localAddress = localAddress || DEFAULT_IPV6_ADDR;
      err = self._handle.bind6(localAddress, localPort, flags);
    }

    err = checkBindError(err, localPort, self._handle);
    if (err) {
      const ex = exceptionWithHostPort(err, 'bind', localAddress, localPort);
      self.destroy(ex);
      return;
    }
  }

  if (addressType === 6 || addressType === 4) {
    const req = new TCPConnectWrap();
    req.oncomplete = afterConnect;
    req.address = address;
    req.port = port;
    req.localAddress = localAddress;
    req.localPort = localPort;

    if (addressType === 4)
      err = self._handle.connect(req, address, port);
    else
      err = self._handle.connect6(req, address, port);
  } else {
    const req = new PipeConnectWrap();
    req.address = address;
    req.oncomplete = afterConnect;

    err = self._handle.connect(req, address, afterConnect);
  }

  if (err) {
    const sockname = self._getsockname();
    let details;

    if (sockname) {
      details = sockname.address + ':' + sockname.port;
    }

    const ex = exceptionWithHostPort(err, 'connect', address, port, details);
    self.destroy(ex);
  }
}

function lookupAndConnect(self, options) {
  const { localAddress, localPort } = options;
  const host = options.host || 'localhost';
  let { port } = options;

  if (localAddress && !isIP(localAddress)) {
    throw new ERR_INVALID_IP_ADDRESS(localAddress);
  }

  if (localPort && typeof localPort !== 'number') {
    throw new ERR_INVALID_ARG_TYPE('options.localPort', 'number', localPort);
  }

  if (typeof port !== 'undefined') {
    if (typeof port !== 'number' && typeof port !== 'string') {
      throw new ERR_INVALID_ARG_TYPE('options.port',
        ['number', 'string'], port);
    }
    validatePort(port);
  }
  port |= 0;

  // If host is an IP, skip performing a lookup
  const addressType = isIP(host);
  if (addressType) {
    defaultTriggerAsyncIdScope(self[async_id_symbol], process.nextTick, () => {
      if (self.connecting)
        defaultTriggerAsyncIdScope(
          self[async_id_symbol],
          internalConnect,
          self, host, port, addressType, localAddress, localPort
        );
    });
    return;
  }

  if (options.lookup && typeof options.lookup !== 'function')
    throw new ERR_INVALID_ARG_TYPE('options.lookup',
      'Function', options.lookup);


  if (dns === undefined) dns = require('dns');
  const dnsopts = {
    family: options.family,
    hints: options.hints || 0
  };

  if (!isWindows &&
    dnsopts.family !== 4 &&
    dnsopts.family !== 6 &&
    dnsopts.hints === 0) {
    dnsopts.hints = dns.ADDRCONFIG;
  }

  self._host = host;
  const lookup = options.lookup || dns.lookup;
  defaultTriggerAsyncIdScope(self[async_id_symbol], function() {
    lookup(host, dnsopts, function emitLookup(err, ip, addressType) {
      self.emit('lookup', err, ip, addressType, host);

      // It's possible we were destroyed while looking this up.
      // XXX it would be great if we could cancel the promise returned by
      // the look up.
      if (!self.connecting) return;

      if (err) {
        // net.createConnection() creates a net.Socket object and immediately
        // calls net.Socket.connect() on it (that's us). There are no event
        // listeners registered yet so defer the error event to the next tick.
        process.nextTick(connectErrorNT, self, err);
      } else if (!isIP(ip)) {
        err = new ERR_INVALID_IP_ADDRESS(ip);
        process.nextTick(connectErrorNT, self, err);
      } else if (addressType !== 4 && addressType !== 6) {
        err = new ERR_INVALID_ADDRESS_FAMILY(addressType,
          options.host,
          options.port);
        process.nextTick(connectErrorNT, self, err);
      } else {
        self._unrefTimer();
        defaultTriggerAsyncIdScope(
          self[async_id_symbol],
          internalConnect,
          self, ip, port, addressType, localAddress, localPort
        );
      }
    });
  });
}

function connectErrorNT(self, err) {
  self.destroy(err);
}

function afterConnect(status, handle, req, readable, writable) {
  const self = handle[owner_symbol];

  // Callback may come after call to destroy
  if (self.destroyed) {
    return;
  }

  assert(self.connecting);
  self.connecting = false;
  self._sockname = null;

  if (status === 0) {
    if (self.readable && !readable) {
      self.push(null);
      self.read();
    }
    if (self.writable && !writable) {
      self.end();
    }
    self._unrefTimer();

    self.emit('connect');
    self.emit('ready');

    // Start the first read, or get an immediate EOF.
    // this doesn't actually consume any bytes, because len=0.
    if (readable && !self.isPaused())
      self.read(0);

  } else {
    self.connecting = false;
    let details;
    if (req.localAddress && req.localPort) {
      details = req.localAddress + ':' + req.localPort;
    }
    const ex = exceptionWithHostPort(status,
      'connect',
      req.address,
      req.port,
      details);
    if (details) {
      ex.localAddress = req.localAddress;
      ex.localPort = req.localPort;
    }
    self.destroy(ex);
  }
}

function toNumber(x: unknown) {
  return (x = Number(x)) >= 0 ? x : false;
}

function emitCloseNT(self: Socket | EventEmitter) {
  self.emit('close');
}
