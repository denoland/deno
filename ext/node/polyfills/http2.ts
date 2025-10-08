// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";

const {
  ArrayFrom,
  ArrayIsArray,
  MathMin,
  Number,
  ObjectAssign,
  ObjectDefineProperty,
  ObjectEntries,
  ObjectHasOwn,
  Promise,
  Proxy,
  ReflectApply,
  ReflectGet,
  ReflectGetPrototypeOf,
  ReflectSet,
  SafeMap,
  SafeSet,
  Symbol,
  SymbolAsyncDispose,
  SymbolDispose,
  Uint32Array,
  Uint8Array,
} = primordials;

import net from "node:net";
import assert from "node:assert";
import http from "node:http";
import tls from "node:tls";
import { EventEmitter } from "node:events";
import { format } from "node:util";
import {
  validateArray,
  validateBuffer,
  validateFunction,
  validateInt32,
  validateNumber,
  validateObject,
  validateString,
  validateUint32,
} from "ext:deno_node/internal/validators.mjs";
import { promisify } from "ext:deno_node/internal/util.mjs";
import {
  ERR_HTTP2_ALTSVC_INVALID_ORIGIN,
  ERR_HTTP2_ALTSVC_LENGTH,
  ERR_HTTP2_INVALID_ORIGIN,
  ERR_HTTP2_INVALID_SESSION,
  ERR_HTTP2_ORIGIN_LENGTH,
  ERR_HTTP2_PING_LENGTH,
  ERR_HTTP2_SESSION_ERROR,
  ERR_HTTP2_SOCKET_BOUND,
  ERR_HTTP2_TOO_MANY_CUSTOM_SETTINGS,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_CHAR,
  ERR_OUT_OF_RANGE,
  hideStackFrames,
} from "ext:deno_node/internal/errors.ts";
import { debuglog } from "ext:deno_node/internal/util/debuglog.ts";
let debug = debuglog("http2", (fn) => {
  debug = fn;
});

// HTTP2 Constants
const MAX_ADDITIONAL_SETTINGS = 10;
const kMaxStreams = 2 ** 31 - 1;
const kMaxALTSVC = 16382;

// NGHTTP2 Constants
const NGHTTP2_NO_ERROR = 0;
const NGHTTP2_INTERNAL_ERROR = 2;
const NGHTTP2_ERR_NOMEM = -901;

// Session type constants
const NGHTTP2_SESSION_SERVER = 0;
const NGHTTP2_SESSION_CLIENT = 1;

// Session flags
const SESSION_FLAGS_PENDING = 0x1;
const SESSION_FLAGS_READY = 0x2;
const SESSION_FLAGS_CLOSED = 0x4;
const SESSION_FLAGS_DESTROYED = 0x8;

// Session field indices
const kSessionUint8FieldCount = 1;
const kSessionRemoteSettingsIsUpToDate = 0;
const kSessionPriorityListenerCount = 0;
const kBitfield = 0;

// Private symbols
const kOptions = Symbol("kOptions");
const kSessions = Symbol("kSessions");
const kBoundSession = Symbol("kBoundSession");
const kState = Symbol("kState");
const kEncrypted = Symbol("kEncrypted");
const kAlpnProtocol = Symbol("kAlpnProtocol");
const kType = Symbol("kType");
const kProxySocket = Symbol("kProxySocket");
const kSocket = Symbol("kSocket");
const kTimeout = Symbol("kTimeout");
const kHandle = Symbol("kHandle");
const kNativeFields = Symbol("kNativeFields");
const kLocalSettings = Symbol("kLocalSettings");
const kRemoteSettings = Symbol("kRemoteSettings");
const kServer = Symbol("kServer");
const kIncomingMessage = Symbol("kIncomingMessage");
const kServerResponse = Symbol("kServerResponse");
const kUpdateTimer = Symbol("kUpdateTimer");
const kMaybeDestroy = Symbol("kMaybeDestroy");
const kInspect = Symbol("kInspect");

// Regular expressions
const kQuotedString = /^[\x20-\x21\x23-\x5B\x5D-\x7E]*$/;

// Placeholder classes and functions - these need proper implementation
class Http2ServerRequest {}
class Http2ServerResponse {}
class JSStreamSocket {}

// Debugging functions
function debugSession(type, message) {
  debug(`session ${type}: ${message}`);
}

function debugSessionObj(session, message) {
  debug(`session ${session[kType]}: ${message}`);
}

// Validates that priority options are correct, specifically:
// 1. options.weight must be a number
// 2. options.parent must be a positive number
// 3. options.exclusive must be a boolean
// 4. if specified, options.silent must be a boolean
//
// Also sets the default priority options if they are not set.
const setAndValidatePriorityOptions = hideStackFrames((options) => {
  deprecateWeight(options);

  if (options.parent === undefined) {
    options.parent = 0;
  } else {
    validateNumber.withoutStackTrace(options.parent, "options.parent", 0);
  }

  if (options.exclusive === undefined) {
    options.exclusive = false;
  } else {
    validateBoolean.withoutStackTrace(options.exclusive, "options.exclusive");
  }

  if (options.silent === undefined) {
    options.silent = false;
  } else {
    validateBoolean.withoutStackTrace(options.silent, "options.silent");
  }
});

// When an error occurs internally at the binding level, immediately
// destroy the session.
function onSessionInternalError(integerCode, customErrorCode) {
  if (this[kOwner] !== undefined) {
    this[kOwner].destroy(new NghttpError(integerCode, customErrorCode));
  }
}

function settingsCallback(cb, ack, duration) {
  this[kState].pendingAck--;
  this[kLocalSettings] = undefined;
  if (ack) {
    debugSessionObj(this, "settings received");
    const settings = this.localSettings;
    if (typeof cb === "function") {
      cb(null, settings, duration);
    }
    this.emit("localSettings", settings);
  } else {
    debugSessionObj(this, "settings canceled");
    if (typeof cb === "function") {
      cb(new ERR_HTTP2_SETTINGS_CANCEL());
    }
  }
}

// Submits a SETTINGS frame to be sent to the remote peer.
function submitSettings(settings, callback) {
  if (this.destroyed) {
    return;
  }
  debugSessionObj(this, "submitting settings");
  this[kUpdateTimer]();
  updateSettingsBuffer(settings);
  if (!this[kHandle].settings(settingsCallback.bind(this, callback))) {
    this.destroy(new ERR_HTTP2_MAX_PENDING_SETTINGS_ACK());
  }
}

// Submit a GOAWAY frame to be sent to the remote peer.
// If the lastStreamID is set to <= 0, then the lastProcStreamID will
// be used. The opaqueData must either be a typed array or undefined
// (which will be checked elsewhere).
function submitGoaway(code, lastStreamID, opaqueData) {
  if (this.destroyed) {
    return;
  }
  debugSessionObj(this, "submitting goaway");
  this[kUpdateTimer]();
  this[kHandle].goaway(code, lastStreamID, opaqueData);
}

const proxySocketHandler = {
  get(session, prop) {
    switch (prop) {
      case "setTimeout":
      case "ref":
      case "unref":
        return session[prop].bind(session);
      case "destroy":
      case "emit":
      case "end":
      case "pause":
      case "read":
      case "resume":
      case "write":
      case "setEncoding":
      case "setKeepAlive":
      case "setNoDelay":
        throw new ERR_HTTP2_NO_SOCKET_MANIPULATION();
      default: {
        const socket = session[kSocket];
        if (socket === undefined) {
          throw new ERR_HTTP2_SOCKET_UNBOUND();
        }
        const value = socket[prop];
        return typeof value === "function" ? value.bind(socket) : value;
      }
    }
  },
  getPrototypeOf(session) {
    const socket = session[kSocket];
    if (socket === undefined) {
      throw new ERR_HTTP2_SOCKET_UNBOUND();
    }
    return ReflectGetPrototypeOf(socket);
  },
  set(session, prop, value) {
    switch (prop) {
      case "setTimeout":
      case "ref":
      case "unref":
        session[prop] = value;
        return true;
      case "destroy":
      case "emit":
      case "end":
      case "pause":
      case "read":
      case "resume":
      case "write":
      case "setEncoding":
      case "setKeepAlive":
      case "setNoDelay":
        throw new ERR_HTTP2_NO_SOCKET_MANIPULATION();
      default: {
        const socket = session[kSocket];
        if (socket === undefined) {
          throw new ERR_HTTP2_SOCKET_UNBOUND();
        }
        socket[prop] = value;
        return true;
      }
    }
  },
};

// pingCallback() returns a function that is invoked when an HTTP2 PING
// frame acknowledgement is received. The ack is either true or false to
// indicate if the ping was successful or not. The duration indicates the
// number of milliseconds elapsed since the ping was sent and the ack
// received. The payload is a Buffer containing the 8 bytes of payload
// data received on the PING acknowledgement.
function pingCallback(cb) {
  return function pingCallback(ack, duration, payload) {
    if (ack) {
      cb(null, duration, payload);
    } else {
      cb(new ERR_HTTP2_PING_CANCEL());
    }
  };
}

// Validates the values in a settings object. Specifically:
// 1. headerTableSize must be a number in the range 0 <= n <= kMaxInt
// 2. initialWindowSize must be a number in the range 0 <= n <= kMaxInt
// 3. maxFrameSize must be a number in the range 16384 <= n <= kMaxFrameSize
// 4. maxConcurrentStreams must be a number in the range 0 <= n <= kMaxStreams
// 5. maxHeaderListSize must be a number in the range 0 <= n <= kMaxInt
// 6. enablePush must be a boolean
// 7. enableConnectProtocol must be a boolean
// All settings are optional and may be left undefined
const validateSettings = hideStackFrames((settings) => {
  if (settings === undefined) return;
  assertIsObject.withoutStackTrace(
    settings.customSettings,
    "customSettings",
    "Number",
  );
  if (settings.customSettings) {
    const entries = ObjectEntries(settings.customSettings);
    if (entries.length > MAX_ADDITIONAL_SETTINGS) {
      throw new ERR_HTTP2_TOO_MANY_CUSTOM_SETTINGS();
    }
    for (const { 0: key, 1: value } of entries) {
      assertWithinRange.withoutStackTrace(
        "customSettings:id",
        Number(key),
        0,
        0xffff,
      );
      assertWithinRange.withoutStackTrace(
        "customSettings:value",
        Number(value),
        0,
        kMaxInt,
      );
    }
  }

  assertWithinRange.withoutStackTrace(
    "headerTableSize",
    settings.headerTableSize,
    0,
    kMaxInt,
  );
  assertWithinRange.withoutStackTrace(
    "initialWindowSize",
    settings.initialWindowSize,
    0,
    kMaxInt,
  );
  assertWithinRange.withoutStackTrace(
    "maxFrameSize",
    settings.maxFrameSize,
    16384,
    kMaxFrameSize,
  );
  assertWithinRange.withoutStackTrace(
    "maxConcurrentStreams",
    settings.maxConcurrentStreams,
    0,
    kMaxStreams,
  );
  assertWithinRange.withoutStackTrace(
    "maxHeaderListSize",
    settings.maxHeaderListSize,
    0,
    kMaxInt,
  );
  assertWithinRange.withoutStackTrace(
    "maxHeaderSize",
    settings.maxHeaderSize,
    0,
    kMaxInt,
  );
  if (
    settings.enablePush !== undefined &&
    typeof settings.enablePush !== "boolean"
  ) {
    throw new ERR_HTTP2_INVALID_SETTING_VALUE.HideStackFramesError(
      "enablePush",
      settings.enablePush,
    );
  }
  if (
    settings.enableConnectProtocol !== undefined &&
    typeof settings.enableConnectProtocol !== "boolean"
  ) {
    throw new ERR_HTTP2_INVALID_SETTING_VALUE.HideStackFramesError(
      "enableConnectProtocol",
      settings.enableConnectProtocol,
    );
  }
});

// Wrap a typed array in a proxy, and allow selectively copying the entries
// that have explicitly been set to another typed array.
function trackAssignmentsTypedArray(typedArray) {
  const typedArrayLength = typedArray.length;
  const modifiedEntries = new Uint8Array(typedArrayLength);

  function copyAssigned(target) {
    for (let i = 0; i < typedArrayLength; i++) {
      if (modifiedEntries[i]) {
        target[i] = typedArray[i];
      }
    }
  }

  return new Proxy(typedArray, {
    __proto__: null,
    get(obj, prop, receiver) {
      if (prop === "copyAssigned") {
        return copyAssigned;
      }
      return ReflectGet(obj, prop, receiver);
    },
    set(obj, prop, value) {
      if (`${+prop}` === prop) {
        modifiedEntries[prop] = 1;
      }
      return ReflectSet(obj, prop, value);
    },
  });
}

// Creates the internal binding.Http2Session handle for an Http2Session
// instance. This occurs only after the socket connection has been
// established. Note: the binding.Http2Session will take over ownership
// of the socket. No other code should read from or write to the socket.
function setupHandle(socket, type, options) {
  // If the session has been destroyed, go ahead and emit 'connect',
  // but do nothing else. The various on('connect') handlers set by
  // core will check for session.destroyed before progressing, this
  // ensures that those at least get cleared out.
  if (this.destroyed) {
    process.nextTick(emit, this, "connect", this, socket);
    return;
  }

  assert(
    socket._handle !== undefined,
    "Internal HTTP/2 Failure. The socket is not connected. Please " +
      "report this as a bug in Node.js",
  );

  debugSession(type, "setting up session handle");
  this[kState].flags |= SESSION_FLAGS_READY;

  updateOptionsBuffer(options);
  if (options.remoteCustomSettings) {
    remoteCustomSettingsToBuffer(options.remoteCustomSettings);
  }
  const handle = new binding.Http2Session(type);
  handle[kOwner] = this;

  handle.consume(socket._handle);
  handle.ongracefulclosecomplete = this[kMaybeDestroy].bind(this, null);

  this[kHandle] = handle;
  if (this[kNativeFields]) {
    // If some options have already been set before the handle existed, copy
    // those (and only those) that have manually been set over.
    this[kNativeFields].copyAssigned(handle.fields);
  }

  this[kNativeFields] = handle.fields;

  if (socket.encrypted) {
    this[kAlpnProtocol] = socket.alpnProtocol;
    this[kEncrypted] = true;
  } else {
    // 'h2c' is the protocol identifier for HTTP/2 over plain-text. We use
    // it here to identify any session that is not explicitly using an
    // encrypted socket.
    this[kAlpnProtocol] = "h2c";
    this[kEncrypted] = false;
  }

  if (isUint32(options.maxSessionInvalidFrames)) {
    const uint32 = new Uint32Array(
      this[kNativeFields].buffer,
      kSessionMaxInvalidFrames,
      1,
    );
    uint32[0] = options.maxSessionInvalidFrames;
  }

  if (isUint32(options.maxSessionRejectedStreams)) {
    const uint32 = new Uint32Array(
      this[kNativeFields].buffer,
      kSessionMaxRejectedStreams,
      1,
    );
    uint32[0] = options.maxSessionRejectedStreams;
  }

  const settings = typeof options.settings === "object" ? options.settings : {};

  this.settings(settings);

  if (
    type === NGHTTP2_SESSION_SERVER &&
    ArrayIsArray(options.origins)
  ) {
    ReflectApply(this.origin, this, options.origins);
  }

  process.nextTick(emit, this, "connect", this, socket);
}

// Emits a close event followed by an error event if err is truthy. Used
// by Http2Session.prototype.destroy()
function emitClose(self, error) {
  if (error) {
    self.emit("error", error);
  }
  self.emit("close");
}

function cleanupSession(session) {
  const socket = session[kSocket];
  const handle = session[kHandle];
  const server = session[kServer];
  session[kProxySocket] = undefined;
  session[kSocket] = undefined;
  session[kHandle] = undefined;
  if (server) {
    server[kSessions].delete(session);
  }
  session[kNativeFields] = trackAssignmentsTypedArray(
    new Uint8Array(kSessionUint8FieldCount),
  );
  if (handle) {
    handle.ondone = null;
  }
  if (socket) {
    socket[kBoundSession] = undefined;
    socket[kServer] = undefined;
  }
}

function finishSessionClose(session, error) {
  debugSessionObj(session, "finishSessionClose");

  const socket = session[kSocket];
  cleanupSession(session);

  if (socket && !socket.destroyed) {
    socket.on("close", () => {
      emitClose(session, error);
    });
    if (session.closed) {
      // If we're gracefully closing the socket, call resume() so we can detect
      // the peer closing in case binding.Http2Session is already gone.
      socket.resume();
    }

    // Always wait for writable side to finish.
    socket.end((err) => {
      debugSessionObj(session, "finishSessionClose socket end", err, error);
      // If session.destroy() was called, destroy the underlying socket. Delay
      // it a bit to try to avoid ECONNRESET on Windows.
      if (!session.closed) {
        setImmediate(() => {
          socket.destroy(error);
        });
      }
    });
  } else {
    process.nextTick(emitClose, session, error);
  }
}

function closeSession(session, code, error) {
  debugSessionObj(session, "start closing/destroying", error);

  const state = session[kState];
  state.flags |= SESSION_FLAGS_DESTROYED;
  state.destroyCode = code;

  // Clear timeout and remove timeout listeners.
  session.setTimeout(0);
  session.removeAllListeners("timeout");

  // Destroy any pending and open streams
  if (state.pendingStreams.size > 0 || state.streams.size > 0) {
    const cancel = new ERR_HTTP2_STREAM_CANCEL(error);
    state.pendingStreams.forEach((stream) => stream.destroy(cancel));
    state.streams.forEach((stream) => stream.destroy(error));
  }

  // Disassociate from the socket and server.
  const socket = session[kSocket];
  const handle = session[kHandle];

  // Destroy the handle if it exists at this point.
  if (handle !== undefined) {
    handle.ondone = finishSessionClose.bind(null, session, error);
    handle.destroy(code, socket.destroyed);
  } else {
    finishSessionClose(session, error);
  }
}

// When the socket emits an error, destroy the associated Http2Session and
// forward it the same error.
function socketOnError(error) {
  const session = this[kBoundSession];
  if (session !== undefined) {
    // We can ignore ECONNRESET after GOAWAY was received as there's nothing
    // we can do and the other side is fully within its rights to do so.
    if (error.code === "ECONNRESET" && session[kState].goawayCode !== null) {
      return session.destroy();
    }
    debugSessionObj(this, "socket error [%s]", error.message);
    session.destroy(error);
  }
}

function socketOnClose() {
  const session = this[kBoundSession];
  if (session !== undefined) {
    debugSessionObj(session, "socket closed");
    const err = session.connecting ? new ERR_SOCKET_CLOSED() : null;
    const state = session[kState];
    state.streams.forEach((stream) => stream.close(NGHTTP2_CANCEL));
    state.pendingStreams.forEach((stream) => stream.close(NGHTTP2_CANCEL));
    session.close();
    closeSession(session, NGHTTP2_NO_ERROR, err);
  }
}

// Upon creation, the Http2Session takes ownership of the socket. The session
// may not be ready to use immediately if the socket is not yet fully connected.
// In that case, the Http2Session will wait for the socket to connect. Once
// the Http2Session is ready, it will emit its own 'connect' event.
//
// The Http2Session.goaway() method will send a GOAWAY frame, signalling
// to the connected peer that a shutdown is in progress. Sending a goaway
// frame has no other effect, however.
//
// Receiving a GOAWAY frame will cause the Http2Session to first emit a 'goaway'
// event notifying the user that a shutdown is in progress. If the goaway
// error code equals 0 (NGHTTP2_NO_ERROR), session.close() will be called,
// causing the Http2Session to send its own GOAWAY frame and switch itself
// into a graceful closing state. In this state, new inbound or outbound
// Http2Streams will be rejected. Existing *pending* streams (those created
// but without an assigned stream ID or handle) will be destroyed with a
// cancel error. Existing open streams will be permitted to complete on their
// own. Once all existing streams close, session.destroy() will be called
// automatically.
//
// Calling session.destroy() will tear down the Http2Session immediately,
// making it no longer usable. Pending and existing streams will be destroyed.
// The bound socket will be destroyed. Once all resources have been freed up,
// the 'close' event will be emitted. Note that pending streams will be
// destroyed using a specific "ERR_HTTP2_STREAM_CANCEL" error. Existing open
// streams will be destroyed using the same error passed to session.destroy()
//
// If destroy is called with an error, an 'error' event will be emitted
// immediately following the 'close' event.
//
// The socket and Http2Session lifecycles are tightly bound. Once one is
// destroyed, the other should also be destroyed. When the socket is destroyed
// with an error, session.destroy() will be called with that same error.
// Likewise, when session.destroy() is called with an error, the same error
// will be sent to the socket.
class Http2Session extends EventEmitter {
  constructor(type, options, socket) {
    super();

    // No validation is performed on the input parameters because this
    // constructor is not exported directly for users.

    // If the session property already exists on the socket,
    // then it has already been bound to an Http2Session instance
    // and cannot be attached again.
    if (socket[kBoundSession] !== undefined) {
      throw new ERR_HTTP2_SOCKET_BOUND();
    }

    socket[kBoundSession] = this;

    if (!socket._handle) {
      socket = new JSStreamSocket(socket);
    }
    socket.on("error", socketOnError);
    socket.on("close", socketOnClose);

    this[kState] = {
      destroyCode: NGHTTP2_NO_ERROR,
      flags: SESSION_FLAGS_PENDING,
      goawayCode: null,
      goawayLastStreamID: null,
      streams: new SafeMap(),
      pendingStreams: new SafeSet(),
      pendingAck: 0,
      shutdownWritableCalled: false,
      writeQueueSize: 0,
      originSet: undefined,
    };

    this[kEncrypted] = undefined;
    this[kAlpnProtocol] = undefined;
    this[kType] = type;
    this[kProxySocket] = null;
    this[kSocket] = socket;
    this[kTimeout] = null;
    this[kHandle] = undefined;

    // Do not use nagle's algorithm
    if (typeof socket.setNoDelay === "function") {
      socket.setNoDelay();
    }

    // Disable TLS renegotiation on the socket
    if (typeof socket.disableRenegotiation === "function") {
      socket.disableRenegotiation();
    }

    const setupFn = setupHandle.bind(this, socket, type, options);
    if (socket.connecting || socket.secureConnecting) {
      const connectEvent = socket instanceof tls.TLSSocket
        ? "secureConnect"
        : "connect";
      socket.once(connectEvent, () => {
        try {
          setupFn();
        } catch (error) {
          socket.destroy(error);
        }
      });
    } else {
      setupFn();
    }

    this[kNativeFields] ||= trackAssignmentsTypedArray(
      new Uint8Array(kSessionUint8FieldCount),
    );
    this.on("newListener", sessionListenerAdded);
    this.on("removeListener", sessionListenerRemoved);

    // Process data on the next tick - a remoteSettings handler may be attached.
    // https://github.com/nodejs/node/issues/35981
    process.nextTick(() => {
      // Socket already has some buffered data - emulate receiving it
      // https://github.com/nodejs/node/issues/35475
      // https://github.com/nodejs/node/issues/34532
      if (socket.readableLength) {
        let buf;
        while ((buf = socket.read()) !== null) {
          debugSession(type, `${buf.length} bytes already in buffer`);
          this[kHandle].receive(buf);
        }
      }
    });

    debugSession(type, "created");
  }

  // Returns undefined if the socket is not yet connected, true if the
  // socket is a TLSSocket, and false if it is not.
  get encrypted() {
    return this[kEncrypted];
  }

  // Returns undefined if the socket is not yet connected, `h2` if the
  // socket is a TLSSocket and the alpnProtocol is `h2`, or `h2c` if the
  // socket is not a TLSSocket.
  get alpnProtocol() {
    return this[kAlpnProtocol];
  }

  // TODO(jasnell): originSet is being added in preparation for ORIGIN frame
  // support. At the current time, the ORIGIN frame specification is awaiting
  // publication as an RFC and is awaiting implementation in nghttp2. Once
  // added, an ORIGIN frame will add to the origins included in the origin
  // set. 421 responses will remove origins from the set.
  get originSet() {
    if (!this.encrypted || this.destroyed) {
      return undefined;
    }
    return ArrayFrom(initOriginSet(this));
  }

  // True if the Http2Session is still waiting for the socket to connect
  get connecting() {
    return (this[kState].flags & SESSION_FLAGS_READY) === 0;
  }

  // True if Http2Session.prototype.close() has been called
  get closed() {
    return !!(this[kState].flags & SESSION_FLAGS_CLOSED);
  }

  // True if Http2Session.prototype.destroy() has been called
  get destroyed() {
    return !!(this[kState].flags & SESSION_FLAGS_DESTROYED);
  }

  // Resets the timeout counter
  [kUpdateTimer]() {
    if (this.destroyed) {
      return;
    }
    if (this[kTimeout]) this[kTimeout].refresh();
  }

  // Sets the id of the next stream to be created by this Http2Session.
  // The value must be a number in the range 0 <= n <= kMaxStreams. The
  // value also needs to be larger than the current next stream ID.
  setNextStreamID(id) {
    if (this.destroyed) {
      throw new ERR_HTTP2_INVALID_SESSION();
    }

    validateNumber(id, "id");
    if (id <= 0 || id > kMaxStreams) {
      throw new ERR_OUT_OF_RANGE("id", `> 0 and <= ${kMaxStreams}`, id);
    }
    this[kHandle].setNextStreamID(id);
  }

  // Sets the local window size (local endpoints's window size)
  // Returns 0 if success or throw an exception if NGHTTP2_ERR_NOMEM
  // if the window allocation fails
  setLocalWindowSize(windowSize) {
    if (this.destroyed) {
      throw new ERR_HTTP2_INVALID_SESSION();
    }

    validateInt32(windowSize, "windowSize", 0);
    const ret = this[kHandle].setLocalWindowSize(windowSize);

    if (ret === NGHTTP2_ERR_NOMEM) {
      this.destroy(new Error("HTTP2 session out of memory"));
    }
  }

  // If ping is called while we are still connecting, or after close() has
  // been called, the ping callback will be invoked immediately with a ping
  // cancelled error and a duration of 0.0.
  ping(payload, callback) {
    if (this.destroyed) {
      throw new ERR_HTTP2_INVALID_SESSION();
    }

    if (typeof payload === "function") {
      callback = payload;
      payload = undefined;
    }
    if (payload) {
      validateBuffer(payload, "payload");
    }
    if (payload && payload.length !== 8) {
      throw new ERR_HTTP2_PING_LENGTH();
    }
    validateFunction(callback, "callback");

    const cb = pingCallback(callback);
    if (this.connecting || this.closed) {
      process.nextTick(cb, false, 0.0, payload);
      return;
    }

    return this[kHandle].ping(payload, cb);
  }

  [kInspect](depth, opts) {
    if (typeof depth === "number" && depth < 0) {
      return this;
    }

    const obj = {
      type: this[kType],
      closed: this.closed,
      destroyed: this.destroyed,
      state: this.state,
      localSettings: this.localSettings,
      remoteSettings: this.remoteSettings,
    };
    return `Http2Session ${format(obj)}`;
  }

  // The socket owned by this session
  get socket() {
    const proxySocket = this[kProxySocket];
    if (proxySocket === null) {
      return this[kProxySocket] = new Proxy(this, proxySocketHandler);
    }
    return proxySocket;
  }

  // The session type
  get type() {
    return this[kType];
  }

  // If a GOAWAY frame has been received, gives the error code specified
  get goawayCode() {
    return this[kState].goawayCode || NGHTTP2_NO_ERROR;
  }

  // If a GOAWAY frame has been received, gives the last stream ID reported
  get goawayLastStreamID() {
    return this[kState].goawayLastStreamID || 0;
  }

  // True if the Http2Session is waiting for a settings acknowledgement
  get pendingSettingsAck() {
    return this[kState].pendingAck > 0;
  }

  // Retrieves state information for the Http2Session
  get state() {
    return this.connecting || this.destroyed
      ? {}
      : getSessionState(this[kHandle]);
  }

  // The settings currently in effect for the local peer. These will
  // be updated only when a settings acknowledgement has been received.
  get localSettings() {
    const settings = this[kLocalSettings];
    if (settings !== undefined) {
      return settings;
    }

    if (this.destroyed || this.connecting) {
      return {};
    }

    return this[kLocalSettings] = getSettings(this[kHandle], false); // Local
  }

  // The settings currently in effect for the remote peer.
  get remoteSettings() {
    if (
      this[kNativeFields][kBitfield] &
      (1 << kSessionRemoteSettingsIsUpToDate)
    ) {
      const settings = this[kRemoteSettings];
      if (settings !== undefined) {
        return settings;
      }
    }

    if (this.destroyed || this.connecting) {
      return {};
    }

    this[kNativeFields][kBitfield] |= 1 << kSessionRemoteSettingsIsUpToDate;
    return this[kRemoteSettings] = getSettings(this[kHandle], true); // Remote
  }

  // Submits a SETTINGS frame to be sent to the remote peer.
  settings(settings, callback) {
    if (this.destroyed) {
      throw new ERR_HTTP2_INVALID_SESSION();
    }
    validateObject(settings, "settings");
    validateSettings(settings);

    if (callback) {
      validateFunction(callback, "callback");
    }
    debugSessionObj(this, "sending settings");

    this[kState].pendingAck++;

    const settingsFn = submitSettings.bind(this, { ...settings }, callback);
    if (this.connecting) {
      this.once("connect", settingsFn);
      return;
    }
    settingsFn();
  }

  // Submits a GOAWAY frame to be sent to the remote peer. Note that this
  // is only a notification, and does not affect the usable state of the
  // session with the notable exception that new incoming streams will
  // be rejected automatically.
  goaway(code = NGHTTP2_NO_ERROR, lastStreamID = 0, opaqueData) {
    if (this.destroyed) {
      throw new ERR_HTTP2_INVALID_SESSION();
    }

    if (opaqueData !== undefined) {
      validateBuffer(opaqueData, "opaqueData");
    }
    validateNumber(code, "code");
    validateNumber(lastStreamID, "lastStreamID");

    const goawayFn = submitGoaway.bind(this, code, lastStreamID, opaqueData);
    if (this.connecting) {
      this.once("connect", goawayFn);
      return;
    }
    goawayFn();
  }

  // Destroy the Http2Session, making it no longer usable and cancelling
  // any pending activity.
  destroy(error = NGHTTP2_NO_ERROR, code) {
    if (this.destroyed) {
      return;
    }

    debugSessionObj(this, "destroying");

    if (typeof error === "number") {
      code = error;
      error = code !== NGHTTP2_NO_ERROR
        ? new ERR_HTTP2_SESSION_ERROR(code)
        : undefined;
    }
    if (code === undefined && error != null) {
      code = NGHTTP2_INTERNAL_ERROR;
    }

    closeSession(this, code, error);
  }

  // Closing the session will:
  // 1. Send a goaway frame
  // 2. Mark the session as closed
  // 3. Prevent new inbound or outbound streams from being opened
  // 4. Optionally register a 'close' event handler
  // 5. Will cause the session to automatically destroy after the
  //    last currently open Http2Stream closes.
  //
  // Close always assumes a good, non-error shutdown (NGHTTP_NO_ERROR)
  //
  // If the session has not connected yet, the closed flag will still be
  // set but the goaway will not be sent until after the connect event
  // is emitted.
  close(callback) {
    if (this.closed || this.destroyed) {
      return;
    }
    debugSessionObj(this, "marking session closed");
    this[kState].flags |= SESSION_FLAGS_CLOSED;
    if (typeof callback === "function") {
      this.once("close", callback);
    }
    this.goaway();
    const handle = this[kHandle];
    if (handle) {
      handle.setGracefulClose();
    }
    this[kMaybeDestroy]();
  }

  [EventEmitter.captureRejectionSymbol](err, event, ...args) {
    switch (event) {
      case "stream": {
        const stream = args[0];
        stream.destroy(err);
        break;
      }
      default:
        this.destroy(err);
    }
  }

  // Destroy the session if:
  // * error is not undefined/null
  // * session is closed and there are no more pending or open streams
  [kMaybeDestroy](error) {
    if (error == null) {
      const handle = this[kHandle];
      const hasPendingData = !!handle && handle.hasPendingData();
      const state = this[kState];
      // Do not destroy if we're not closed and there are pending/open streams
      if (
        !this.closed ||
        state.streams.size > 0 ||
        state.pendingStreams.size > 0 || hasPendingData
      ) {
        return;
      }
    }
    this.destroy(error);
  }

  _onTimeout() {
    callTimeout(this, this);
  }

  ref() {
    if (this[kSocket]) {
      this[kSocket].ref();
    }
  }

  unref() {
    if (this[kSocket]) {
      this[kSocket].unref();
    }
  }
}

// ServerHttp2Session instances should never have to wait for the socket
// to connect as they are always created after the socket has already been
// established.
class ServerHttp2Session extends Http2Session {
  constructor(options, socket, server) {
    super(NGHTTP2_SESSION_SERVER, options, socket);
    this[kServer] = server;
    if (server) {
      server[kSessions].add(this);
    }
    // This is a bit inaccurate because it does not reflect changes to
    // number of listeners made after the session was created. This should
    // not be an issue in practice. Additionally, the 'priority' event on
    // server instances (or any other object) is fully undocumented.
    this[kNativeFields][kSessionPriorityListenerCount] = server
      ? server.listenerCount("priority")
      : 0;
  }

  get server() {
    return this[kServer];
  }

  // Submits an altsvc frame to be sent to the client. `stream` is a
  // numeric Stream ID. origin is a URL string that will be used to get
  // the origin. alt is a string containing the altsvc details. No fancy
  // API is provided for that.
  altsvc(alt, originOrStream) {
    if (this.destroyed) {
      throw new ERR_HTTP2_INVALID_SESSION();
    }

    let stream = 0;
    let origin;

    if (typeof originOrStream === "string") {
      origin = getURLOrigin(originOrStream);
      if (origin === "null") {
        throw new ERR_HTTP2_ALTSVC_INVALID_ORIGIN();
      }
    } else if (typeof originOrStream === "number") {
      if (originOrStream >>> 0 !== originOrStream || originOrStream === 0) {
        throw new ERR_OUT_OF_RANGE(
          "originOrStream",
          `> 0 && < ${2 ** 32}`,
          originOrStream,
        );
      }
      stream = originOrStream;
    } else if (originOrStream !== undefined) {
      // Allow origin to be passed a URL or object with origin property
      if (originOrStream !== null && typeof originOrStream === "object") {
        origin = originOrStream.origin;
      }
      // Note: if originOrStream is an object with an origin property other
      // than a URL, then it is possible that origin will be malformed.
      // We do not verify that here. Users who go that route need to
      // ensure they are doing the right thing or the payload data will
      // be invalid.
      if (typeof origin !== "string") {
        throw new ERR_INVALID_ARG_TYPE("originOrStream", [
          "string",
          "number",
          "URL",
          "object",
        ], originOrStream);
      } else if (origin === "null" || origin.length === 0) {
        throw new ERR_HTTP2_ALTSVC_INVALID_ORIGIN();
      }
    }

    validateString(alt, "alt");
    if (!kQuotedString.test(alt)) {
      throw new ERR_INVALID_CHAR("alt");
    }

    // Max length permitted for ALTSVC
    if (
      (alt.length + (origin !== undefined ? origin.length : 0)) > kMaxALTSVC
    ) {
      throw new ERR_HTTP2_ALTSVC_LENGTH();
    }

    this[kHandle].altsvc(stream, origin || "", alt);
  }

  // Submits an origin frame to be sent.
  origin(...origins) {
    if (this.destroyed) {
      throw new ERR_HTTP2_INVALID_SESSION();
    }

    if (origins.length === 0) {
      return;
    }

    let arr = "";
    let len = 0;
    const count = origins.length;
    for (let i = 0; i < count; i++) {
      let origin = origins[i];
      if (typeof origin === "string") {
        origin = getURLOrigin(origin);
      } else if (origin != null && typeof origin === "object") {
        origin = origin.origin;
      }
      validateString(origin, "origin");
      if (origin === "null") {
        throw new ERR_HTTP2_INVALID_ORIGIN();
      }

      arr += `${origin}\0`;
      len += origin.length;
    }

    if (len > kMaxALTSVC) {
      throw new ERR_HTTP2_ORIGIN_LENGTH();
    }

    this[kHandle].origin(arr, count);
  }
}

function connectionListener(socket) {
  debug("Http2Session server: received a connection");
  const options = this[kOptions] || {};

  if (socket.alpnProtocol === false || socket.alpnProtocol === "http/1.1") {
    // Fallback to HTTP/1.1
    if (options.allowHTTP1 === true) {
      socket.server[kIncomingMessage] = options.Http1IncomingMessage;
      socket.server[kServerResponse] = options.Http1ServerResponse;
      // TODO
      // return httpConnectionListener.call(this, socket);
    }
    // Let event handler deal with the socket
    debug(
      "Unknown protocol from %s:%s",
      socket.remoteAddress,
      socket.remotePort,
    );
    if (!this.emit("unknownProtocol", socket)) {
      debug("Unknown protocol timeout:  %s", options.unknownProtocolTimeout);
      // Install a timeout if the socket was not successfully closed, then
      // destroy the socket to ensure that the underlying resources are
      // released.
      const timer = setTimeout(() => {
        if (!socket.destroyed) {
          debug("UnknownProtocol socket timeout, destroy socket");
          socket.destroy();
        }
      }, options.unknownProtocolTimeout);
      // Un-reference the timer to avoid blocking of application shutdown and
      // clear the timeout if the socket was successfully closed.
      timer.unref();

      socket.once("close", () => clearTimeout(timer));

      // We don't know what to do, so let's just tell the other side what's
      // going on in a format that they *might* understand.
      socket.end(
        "HTTP/1.0 403 Forbidden\r\n" +
          "Content-Type: text/plain\r\n\r\n" +
          "Missing ALPN Protocol, expected `h2` to be available.\n" +
          "If this is a HTTP request: The server was not " +
          "configured with the `allowHTTP1` option or a " +
          "listener for the `unknownProtocol` event.\n",
      );
    }
    return;
  }

  try {
    // Set up the Session
    const session = new ServerHttp2Session(options, socket, this);

    session.on("stream", sessionOnStream);
    session.on("error", sessionOnError);
    // Don't count our own internal listener.
    session.on("priority", sessionOnPriority);
    session[kNativeFields][kSessionPriorityListenerCount]--;

    if (this.timeout) {
      session.setTimeout(this.timeout, sessionOnTimeout);
    }

    socket[kServer] = this;

    this.emit("session", session);
  } catch (e) {
    console.error(e);
  }
}

function onServerStream(
  ServerRequest,
  ServerResponse,
  stream,
  headers,
  flags,
  rawHeaders,
) {
}

function setupCompat(ev) {
  if (ev === "request") {
    this.removeListener("newListener", setupCompat);
    this.on(
      "stream",
      onServerStream.bind(
        this,
        this[kOptions].Http2ServerRequest,
        this[kOptions].Http2ServerResponse,
      ),
    );
  }
}

function closeAllSessions(server) {
  // TODO: Implement session cleanup
}

function initializeOptions(options) {
  validateObject(options, "options");
  options = { ...options };
  if (options.settings !== undefined) {
    validateObject(options.settings, "options.settings");
    options.settings = { ...options.settings };
  } else {
    options.settings = {};
  }

  if (options.remoteCustomSettings !== undefined) {
    validateArray(options.remoteCustomSettings, "options.remoteCustomSettings");
    options.remoteCustomSettings = [...options.remoteCustomSettings];
    if (options.remoteCustomSettings.length > MAX_ADDITIONAL_SETTINGS) {
      throw new ERR_HTTP2_TOO_MANY_CUSTOM_SETTINGS();
    }
  }

  if (options.maxSessionInvalidFrames !== undefined) {
    validateUint32(options.maxSessionInvalidFrames, "maxSessionInvalidFrames");
  }

  if (options.maxSessionRejectedStreams !== undefined) {
    validateUint32(
      options.maxSessionRejectedStreams,
      "maxSessionRejectedStreams",
    );
  }

  if (options.unknownProtocolTimeout !== undefined) {
    validateUint32(options.unknownProtocolTimeout, "unknownProtocolTimeout");
  } // TODO(danbev): is this a good default value?
  else {
    options.unknownProtocolTimeout = 10000;
  }

  // Used only with allowHTTP1
  options.Http1IncomingMessage ||= http.IncomingMessage;
  options.Http1ServerResponse ||= http.ServerResponse;

  options.Http2ServerRequest ||= Http2ServerRequest;
  options.Http2ServerResponse ||= Http2ServerResponse;
  return options;
}

function createServer(options, handler) {
  if (typeof options === "function") {
    handler = options;
    options = {};
  }
  return new Http2Server(options, handler);
}

class Http2Server extends net.Server {
  constructor(options, requestListener) {
    options = initializeOptions(options);
    super(options, connectionListener);
    this[kOptions] = options;
    this[kSessions] = new SafeSet();
    this.timeout = 0;
    this.on("newListener", setupCompat);
    if (typeof requestListener === "function") {
      this.on("request", requestListener);
    }
  }

  setTimeout(msecs, callback) {
    this.timeout = msecs;
    if (callback !== undefined) {
      validateFunction(callback, "callback");
      this.on("timeout", callback);
    }
    return this;
  }

  updateSettings(settings) {
    validateObject(settings, "settings");
    validateSettings(settings);
    this[kOptions].settings = { ...this[kOptions].settings, ...settings };
  }

  close() {
    ReflectApply(net.Server.prototype.close, this, arguments);
    closeAllSessions(this);
  }

  async [SymbolAsyncDispose]() {
    await promisify(super.close).call(this);
  }
}
export { createServer };

export default { createServer };
