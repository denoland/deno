// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
//
// Ported from Node.js lib/internal/tls/wrap.js

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { core } from "ext:core/mod.js";
import {
  ArrayIsArray,
  ObjectAssign,
  StringPrototypeReplace,
} from "ext:deno_node/internal/primordials.mjs";
import assert from "ext:deno_node/internal/assert.mjs";
import * as net from "node:net";
import {
  createSecureContext,
  translatePeerCertificate,
} from "node:_tls_common";
import { JSStreamSocket } from "ext:deno_node/internal/js_stream_socket.js";
import { convertALPNProtocols } from "ext:deno_node/internal/tls_common.js";
import { Buffer } from "node:buffer";
import {
  connResetException,
  ERR_TLS_CERT_ALTNAME_INVALID,
  ERR_TLS_REQUIRED_SERVER_NAME,
} from "ext:deno_node/internal/errors.ts";
import { debuglog } from "ext:deno_node/internal/util/debuglog.ts";
import {
  constants as TCPConstants,
  TCP,
} from "ext:deno_node/internal_binding/tcp_wrap.ts";
import { kMaybeDestroy } from "ext:deno_node/internal/stream_base_commons.ts";
import {
  constants as PipeConstants,
  Pipe,
} from "ext:deno_node/internal_binding/pipe_wrap.ts";
import { kEmptyObject } from "ext:deno_node/internal/util.mjs";
import { nextTick } from "ext:deno_node/_next_tick.ts";
import {
  validateFunction,
  validateNumber,
  validateObject,
} from "ext:deno_node/internal/validators.mjs";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import { op_tls_canonicalize_ipv4_address } from "ext:core/ops";
import tlsWrap from "ext:deno_node/internal_binding/tls_wrap.ts";
import { ownerSymbol } from "ext:deno_node/internal_binding/symbols.ts";
import { X509Certificate } from "ext:deno_node/internal/crypto/x509.ts";

const kConnectOptions = Symbol("connect-options");
const kHandshakeTimer = Symbol("handshake-timer");
const kIsVerified = Symbol("verified");
const kPendingSession = Symbol("pendingSession");
const kRes = Symbol("res");
const kErrorEmitted = Symbol("error-emitted");
const noop = () => {};

let debug = debuglog("tls", (fn) => {
  debug = fn;
});

function canonicalizeIP(ip) {
  return op_tls_canonicalize_ipv4_address(ip);
}

function toBufferLike(value) {
  if (!value) {
    return undefined;
  }
  if (typeof value === "string") {
    return Buffer.from(value);
  }
  if (isArrayBufferView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  return undefined;
}

function getContextCertValue(socket) {
  const context = socket._tlsOptions?.secureContext?.context ??
    socket._tlsOptions?.secureContext;
  let cert = context?.cert ?? socket._tlsOptions?.cert;
  if (ArrayIsArray(cert)) {
    cert = cert[0];
  }
  return toBufferLike(cert);
}

function setIssuerCertificate(cert, issuer) {
  if (issuer) {
    Object.defineProperty(cert, "issuerCertificate", {
      __proto__: null,
      configurable: true,
      enumerable: true,
      value: issuer,
      writable: false,
    });
  }
  return cert;
}

function getPeerCertificateChain(handle) {
  return handle?.getPeerCertificateChain?.()?.certificates ?? null;
}

function buildPeerLegacyCertificate(handle) {
  const cert = handle?.getPeerCertificate?.(true);
  if (!cert) {
    return {};
  }

  const chain = getPeerCertificateChain(handle);
  if (chain?.length > 1) {
    let current = cert;
    for (let i = 1; i < chain.length; i++) {
      const issuer = new X509Certificate(chain[i]).toLegacyObject();
      current.issuerCertificate = issuer;
      current = issuer;
    }
  }

  return translatePeerCertificate(cert) || {};
}

// ---------------------------------------------------------------------------
// TLSWrap callbacks - called on the native TLSWrap handle (this === handle)
// ---------------------------------------------------------------------------

function onhandshakedone() {
  debug("client onhandshakedone");
  const owner = this._owner;
  if (owner) owner._finishInit();
}

function onerror(err) {
  const owner = this._owner;
  if (!owner) return;

  debug(
    "%s onerror %s had? %j",
    owner._tlsOptions?.isServer ? "server" : "client",
    err,
    owner._hadError,
  );

  if (owner._hadError) return;
  owner._hadError = true;

  if (!owner._secureEstablished) {
    owner._closeAfterHandlingError = true;
    owner.destroy(err);
  } else {
    owner._emitTLSError(err);
  }
}

// ---------------------------------------------------------------------------
// initRead - start data flowing from the TLSWrap handle
// ---------------------------------------------------------------------------
function initRead(tlsSocket, socket) {
  debug(
    "%s initRead",
    tlsSocket._tlsOptions?.isServer ? "server" : "client",
    "handle?",
    !!tlsSocket._handle,
    "buffered?",
    !!socket && socket.readableLength,
  );
  if (!tlsSocket._handle) return;

  // If the underlying socket already has buffered data, feed it to TLSWrap
  if (socket?.readableLength) {
    let buf;
    while ((buf = socket.read()) !== null) {
      tlsSocket._handle.receive(buf);
    }
  }

  tlsSocket.read(0);
}

// ---------------------------------------------------------------------------
// TLSSocket - the main class
// ---------------------------------------------------------------------------

function TLSSocket(socket, opts) {
  const tlsOptions = { ...opts };

  this._tlsOptions = tlsOptions;
  this._secureEstablished = false;
  this._securePending = false;
  this._newSessionPending = false;
  this._controlReleased = false;
  this.secureConnecting = true;
  this._SNICallback = null;
  this.servername = null;
  this.alpnProtocol = null;
  this.authorized = false;
  this.authorizationError = null;
  this[kRes] = null;
  this[kIsVerified] = false;
  this[kPendingSession] = null;
  this._session = null;
  this._sessionReused = false;

  let wrap;
  let handle;

  if (socket) {
    if (socket instanceof net.Socket && socket._handle) {
      wrap = socket;
    } else {
      wrap = new JSStreamSocket(socket);
    }
    handle = wrap._handle;
  } else {
    wrap = null;
  }

  // Just a documented property to make secure sockets
  // distinguishable from regular ones.
  this.encrypted = true;

  // Validate SNICallback early (before _wrapHandle eagerly inits TLS)
  if (tlsOptions.isServer && tlsOptions.SNICallback) {
    validateFunction(tlsOptions.SNICallback, "options.SNICallback");
  }

  net.Socket.call(this, {
    handle: this._wrapHandle(wrap, handle),
    allowHalfOpen: socket ? socket.allowHalfOpen : tlsOptions.allowHalfOpen,
    autoDestroy: true,
    pauseOnCreate: tlsOptions.pauseOnConnect,
    manualStart: true,
    highWaterMark: tlsOptions.highWaterMark,
    onread: !socket ? tlsOptions.onread : null,
    signal: tlsOptions.signal,
  });

  // Proxy for API compatibility
  this.ssl = this._handle;

  this.on("error", this._tlsError);

  this._init(socket, wrap);

  // Implement kMaybeDestroy so that onStreamRead (stream_base_commons.ts)
  // can auto-destroy the socket when EOF is received. Without this, the
  // native TCP handle keeps a ref on the event loop forever.
  this[kMaybeDestroy] = () => {
    if (!this.destroyed) {
      const rDone = this._readableState?.ended || this.readableEnded;
      const wDone = this._writableState?.finished || this.writableFinished;
      if (rDone && wDone) {
        this.destroy();
      }
    }
  };
  this.on("finish", this[kMaybeDestroy]);

  // Read on next tick so the caller has a chance to setup listeners
  nextTick(initRead, this, socket);
}
Object.setPrototypeOf(TLSSocket.prototype, net.Socket.prototype);
Object.setPrototypeOf(TLSSocket, net.Socket);

tlsWrap.TLSWrap.prototype.close = function close(cb) {
  let ssl;
  if (this._owner) {
    ssl = this._owner.ssl;
    this._owner.ssl = null;
  }

  // deno-lint-ignore no-this-alias
  const self = this;
  const done = () => {
    if (ssl) {
      ssl.destroySsl();
    }
    // Close the native TCP handle to remove it from the event loop.
    if (self._nativeTcpHandle) {
      self._nativeTcpHandle.readStop();
      self._nativeTcpHandle.close();
      self._nativeTcpHandle = null;
    }
    if (cb) cb();
  };

  if (this._parentWrap) {
    if (this._parentWrap._handle === null) {
      queueMicrotask(done);
      return;
    }

    if (this._parentWrap._handle === this._parent) {
      this._parentWrap.once("close", done);
      this._parentWrap.destroy();
      return;
    }
  }

  // Defer so callers can register "close" listeners after destroy().
  queueMicrotask(done);
};

TLSSocket.prototype._wrapHandle = function (wrap, handle) {
  const options = this._tlsOptions;
  if (!handle) {
    handle = options.pipe
      ? new Pipe(PipeConstants.SOCKET)
      : new TCP(TCPConstants.SOCKET);
  }

  // Wrap socket's handle with TLSWrap
  const context = options.secureContext ||
    options.credentials ||
    createSecureContext(options);
  const secureContext = {
    ...(context?.context ?? context),
    rejectUnauthorized: options.rejectUnauthorized !== false,
  };

  // Get the native TCP handle for attachment
  const nativeHandle = handle;

  // Derive servername for SNI. Default to host, or the wrapped socket's host.
  let servername = options.servername ?? options.host ?? wrap?._host;
  if (servername && servername.endsWith(".")) {
    servername = servername.slice(0, -1);
  }

  const res = tlsWrap.wrap(
    nativeHandle,
    secureContext,
    !!options.isServer,
    servername,
  );

  res._parent = handle; // C++ "wrap" object: TCPWrap, etc.
  res._parentWrap = wrap; // JS object: net.Socket, etc.
  res._secureContext = context;
  res.reading = handle.reading;
  res._owner = this;
  res[ownerSymbol] = this; // For onWriteComplete to find the socket
  this[kRes] = res;

  // Set ownerSymbol on the parent handle so that connect callbacks
  // (which receive the TCP handle, not the TLSWrap) can find the socket.
  handle[ownerSymbol] = this;

  // Proxy methods from the parent TCP handle that callers expect on _handle.
  // In Node, TLSWrap is a StreamBase that delegates these to the underlying
  // stream. We proxy them explicitly here.
  const proxyMethods = [
    "setNetPermToken",
    "getsockname",
    "getpeername",
    "connect",
    "connect6",
    "bind",
    "bind6",
    "listen",
    "ref",
    "unref",
    "setNoDelay",
    "setKeepAlive",
  ];
  for (const method of proxyMethods) {
    if (typeof handle[method] === "function") {
      res[method] = handle[method].bind(handle);
    }
  }

  // Proxy the reading property
  defineHandleReading(this, handle);

  if (wrap) {
    wrap.on("close", () => this.destroy());
  }

  return res;
};

function defineHandleReading(socket, handle) {
  Object.defineProperty(handle, "reading", {
    __proto__: null,
    get: () => {
      return socket[kRes].reading;
    },
    set: (value) => {
      socket[kRes].reading = value;
    },
  });
}

TLSSocket.prototype._destroySsl = function _destroySsl() {
  if (!this.ssl) return;
  this.ssl.destroySsl();
  this.ssl = null;
  this[kPendingSession] = null;
  this[kIsVerified] = false;
};

TLSSocket.prototype.disableRenegotiation = function disableRenegotiation() {
  // Renegotiation not supported by rustls - this is a no-op
};

// Constructor guts - sets up callbacks on the TLSWrap handle
TLSSocket.prototype._init = function (socket, wrap) {
  const options = this._tlsOptions;
  const ssl = this._handle;
  this.server = options.server;

  debug(
    "%s _init",
    options.isServer ? "server" : "client",
    "handle?",
    !!ssl,
  );

  const requestCert = !!options.requestCert || !options.isServer;
  const rejectUnauthorized = !!options.rejectUnauthorized;

  this._requestCert = requestCert;
  this._rejectUnauthorized = rejectUnauthorized;
  if (requestCert || rejectUnauthorized) {
    ssl.setVerifyMode(requestCert, rejectUnauthorized);
  }

  if (options.isServer) {
    ssl.onhandshakestart = noop;
    ssl.onhandshakedone = function () {
      debug("server onhandshakedone");
      const owner = this._owner;
      if (!owner) return;
      if (owner._newSessionPending) {
        owner._securePending = true;
        return;
      }
      owner._finishInit();
    };
  } else {
    ssl.onhandshakestart = noop;
    ssl.onhandshakedone = onhandshakedone;

    if (options.session) {
      ssl.setSession(options.session);
    }
  }

  if (options.ALPNProtocols) {
    ssl.setAlpnProtocols(options.ALPNProtocols);
  }

  ssl.onerror = onerror;

  // Set SNICallback (already validated in constructor)
  if (options.isServer && options.SNICallback) {
    this._SNICallback = options.SNICallback;
  }

  if (options.handshakeTimeout > 0) {
    this[kHandshakeTimer] = core.createTimer(
      () => {
        core.cancelTimer(this[kHandshakeTimer]);
        this[kHandshakeTimer] = null;
        this._handleTimeout();
      },
      options.handshakeTimeout,
      undefined,
      false,
      false,
      true, // isSystem: avoid test sanitizer detection
    );
  }

  if (socket instanceof net.Socket) {
    this._parent = socket;

    this.connecting = socket.connecting || !socket._handle;
    socket.once("connect", () => {
      this.connecting = false;
      // If the original socket created its own TCP handle during
      // connect() (because it had no handle when we wrapped it),
      // re-attach the TLS wrap to the socket's actual TCP handle.
      if (ssl && socket._handle) {
        const nativeHandle = socket._handle;
        ssl.attach(nativeHandle);
      }
      this.emit("connect");
    });
  }

  if (wrap) {
    wrap.on("error", (err) => this._emitTLSError(err));
  } else {
    assert(!socket);
    this.connecting = true;
  }
};

TLSSocket.prototype.renegotiate = function (_options, callback) {
  // Renegotiation not supported by rustls
  if (callback) {
    nextTick(callback, new Error("Renegotiation not supported"));
  }
  return false;
};

TLSSocket.prototype.setMaxSendFragment = function setMaxSendFragment(_size) {
  // Not applicable to rustls
  return true;
};

TLSSocket.prototype._handleTimeout = function () {
  this._emitTLSError(new Error("TLS handshake timeout"));
};

TLSSocket.prototype._emitTLSError = function (err) {
  const e = this._tlsError(err);
  if (e) this.emit("error", e);
};

TLSSocket.prototype._tlsError = function (err) {
  this.emit("_tlsError", err);
  if (this._controlReleased) return err;
  return null;
};

TLSSocket.prototype._releaseControl = function () {
  if (this._controlReleased) return false;
  this._controlReleased = true;
  this.removeListener("error", this._tlsError);
  return true;
};

TLSSocket.prototype._finishInit = function () {
  if (!this._handle) return;

  try {
    const alpnOut = {};
    this._handle.getAlpnNegotiatedProtocol(alpnOut);
    this.alpnProtocol = alpnOut.alpnProtocol || false;
    if (this.servername === null) {
      this.servername = this._handle.getServername?.() ?? null;
    }
  } catch (_e) {
    // getAlpnNegotiatedProtocol/getServername may not be available
  }

  debug(
    "%s _finishInit",
    this._tlsOptions?.isServer ? "server" : "client",
    "handle?",
    !!this._handle,
    "alpn",
    this.alpnProtocol,
    "servername",
    this.servername,
  );

  this._secureEstablished = true;
  this._tlsUpgraded = true;
  if (this[kHandshakeTimer]) {
    core.cancelTimer(this[kHandshakeTimer]);
    this[kHandshakeTimer] = null;
  }
  this.emit("secure");
};

TLSSocket.prototype._start = function () {
  debug(
    "%s _start",
    this._tlsOptions?.isServer ? "server" : "client",
    "handle?",
    !!this._handle,
    "connecting?",
    this.connecting,
  );
  if (this.connecting) {
    this.once("connect", this._start);
    return;
  }

  if (!this._handle) return;

  this._handle.start();

  // Start reading on the underlying native TCP handle so that encrypted
  // data flows to the TLSWrap via the JS onread interceptor.
  const tcpHandle = this._handle._nativeTcpHandle;
  if (tcpHandle) {
    // readStart caches the onread callback on first call. If it was already
    // called (e.g. by net.Socket.resume), we need to stop and restart to
    // pick up the new interceptor callback.
    tcpHandle.readStop();
    tcpHandle.readStart();
  }
};

TLSSocket.prototype.setServername = function (name) {
  if (typeof name !== "string") {
    throw new TypeError("Server name must be a string");
  }
  if (this._tlsOptions?.isServer) {
    throw new Error("Cannot set servername on a server socket");
  }
  this._handle?.setServername(name);
};

TLSSocket.prototype.setSession = function (_session) {
  if (typeof _session === "string") {
    _session = Buffer.from(_session, "latin1");
  }
  this._session = _session ? Buffer.from(_session) : null;
  // Note: rustls does not support session resumption via setSession.
  // Do not set _sessionReused = true here; the session buffer is stored
  // but not actually sent to the native TLS layer for 0-RTT reuse.
  // Reporting true would mislead connection pooling logic.
};

TLSSocket.prototype.getPeerCertificate = function (detailed) {
  if (!this._handle) {
    return null;
  }
  if (!detailed) {
    const cert = this._handle.getPeerCertificate(false);
    return translatePeerCertificate(cert) || {};
  }
  return buildPeerLegacyCertificate(this._handle);
};

TLSSocket.prototype.getCertificate = function () {
  const cert = getContextCertValue(this);
  if (!cert) {
    return this._handle ? {} : null;
  }
  return translatePeerCertificate(new X509Certificate(cert).toLegacyObject()) ||
    {};
};

TLSSocket.prototype.getEphemeralKeyInfo = function () {
  return {};
};

TLSSocket.prototype.isSessionReused = function () {
  if (this._handle?.isSessionReused) {
    return this._handle.isSessionReused();
  }
  return this._sessionReused;
};

// Proxy TLSSocket handle methods
function makeSocketMethodProxy(name) {
  return function socketMethodProxy(...args) {
    if (this._handle) {
      return this._handle[name]?.(...args);
    }
    return null;
  };
}

TLSSocket.prototype.getCipher = function getCipher() {
  if (!this._handle) {
    return null;
  }
  const out = {};
  return this._handle.getCipher(out) === 0 ? out : null;
};

TLSSocket.prototype.getProtocol = function getProtocol() {
  if (!this._handle) {
    return null;
  }
  const out = {};
  return this._handle.getProtocol(out) === 0 ? out.protocol : null;
};

TLSSocket.prototype.getFinished = function getFinished() {
  const data = this._handle?.getFinished();
  return data
    ? Buffer.from(data.buffer, data.byteOffset, data.byteLength)
    : undefined;
};

TLSSocket.prototype.getPeerFinished = function getPeerFinished() {
  const data = this._handle?.getPeerFinished();
  return data
    ? Buffer.from(data.buffer, data.byteOffset, data.byteLength)
    : undefined;
};

TLSSocket.prototype.getSession = function getSession() {
  return this._session ?? null;
};

TLSSocket.prototype.getPeerX509Certificate = function getPeerX509Certificate() {
  const chain = getPeerCertificateChain(this._handle);
  if (!chain?.length) {
    return undefined;
  }
  const cert = new X509Certificate(chain[0]);
  const issuer = chain[1] ? new X509Certificate(chain[1]) : undefined;
  return setIssuerCertificate(cert, issuer);
};

TLSSocket.prototype.getX509Certificate = function getX509Certificate() {
  const cert = getContextCertValue(this);
  return cert ? new X509Certificate(cert) : undefined;
};

["enableTrace"]
  .forEach((method) => {
    TLSSocket.prototype[method] = makeSocketMethodProxy(method);
  });

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

function makeVerifyError(code) {
  if (!code) return null;
  const err = new Error(code);
  err.code = code;
  return err;
}

function onServerSocketSecure() {
  if (this._requestCert) {
    const verifyError = makeVerifyError(this._handle.verifyError());
    if (verifyError) {
      this.authorizationError = verifyError.code;

      if (this._rejectUnauthorized) {
        this.destroy();
        return;
      }
    } else {
      this.authorized = true;
    }
  } else {
    this.authorized = true;
  }

  if (!this.destroyed && this._releaseControl()) {
    debug("server emit secureConnection");
    this.secureConnecting = false;
    this._tlsOptions.server.emit("secureConnection", this);
  }
}

function onSocketTLSError(err) {
  if (!this._controlReleased && !this[kErrorEmitted]) {
    this[kErrorEmitted] = true;
    debug("server emit tlsClientError:", err);
    this._tlsOptions.server.emit("tlsClientError", err, this);
  }
}

function onSocketClose(err) {
  if (err) return;
  if (!this._controlReleased && !this[kErrorEmitted]) {
    this[kErrorEmitted] = true;
    const connReset = connResetException("socket hang up");
    this._tlsOptions.server.emit("tlsClientError", connReset, this);
  }
}

function tlsConnectionListener(rawSocket) {
  debug("net.Server.on(connection): new TLSSocket");
  const socket = new TLSSocket(rawSocket, {
    secureContext: this._sharedCreds,
    isServer: true,
    server: this,
    requestCert: this.requestCert,
    rejectUnauthorized: this.rejectUnauthorized,
    ALPNProtocols: this.ALPNProtocols,
    SNICallback: this._SNICallback,
    pauseOnConnect: this.pauseOnConnect,
  });

  // Start the TLS handshake for server-side sockets
  socket._start();

  socket.on("secure", onServerSocketSecure);

  socket[kErrorEmitted] = false;
  socket.on("close", onSocketClose);
  socket.on("_tlsError", onSocketTLSError);
  socket.on("error", onSocketTLSError);
}

function Server(options, listener) {
  if (!(this instanceof Server)) {
    return new Server(options, listener);
  }

  if (typeof options === "function") {
    listener = options;
    options = kEmptyObject;
  } else if (options == null || typeof options === "object") {
    options ??= kEmptyObject;
  } else {
    throw new TypeError("options must be an object");
  }

  this._contexts = [];
  this.requestCert = options.requestCert === true;
  this.rejectUnauthorized = options.rejectUnauthorized !== false;

  if (options.ALPNProtocols) {
    convertALPNProtocols(options.ALPNProtocols, this);
  }

  this.setSecureContext(options);
  this._handshakeTimeout = options.handshakeTimeout || (120 * 1000);
  validateNumber(this._handshakeTimeout, "options.handshakeTimeout");

  this._SNICallback = options.SNICallback;

  if (this._SNICallback) {
    validateFunction(this._SNICallback, "options.SNICallback");
  }

  // Constructor call
  net.Server.call(this, options, tlsConnectionListener);

  if (listener) {
    this.on("secureConnection", listener);
  }
}

Object.setPrototypeOf(Server.prototype, net.Server.prototype);
Object.setPrototypeOf(Server, net.Server);

Server.prototype.setSecureContext = function (options) {
  validateObject(options, "options");

  this._sharedCreds = createSecureContext({
    allowPartialTrustChain: options.allowPartialTrustChain,
    ca: options.ca,
    cert: options.cert,
    ciphers: options.ciphers,
    clientCertEngine: options.clientCertEngine,
    crl: options.crl,
    dhparam: options.dhparam,
    ecdhCurve: options.ecdhCurve,
    honorCipherOrder: options.honorCipherOrder !== undefined
      ? !!options.honorCipherOrder
      : true,
    key: options.key,
    maxVersion: options.maxVersion,
    minVersion: options.minVersion,
    passphrase: options.passphrase,
    pfx: options.pfx,
    privateKeyEngine: options.privateKeyEngine,
    privateKeyIdentifier: options.privateKeyIdentifier,
    secureOptions: options.secureOptions,
    secureProtocol: options.secureProtocol,
    sessionIdContext: options.sessionIdContext,
    sessionTimeout: options.sessionTimeout,
    sigalgs: options.sigalgs,
    ticketKeys: options.ticketKeys,
  });
};

Server.prototype.addContext = function (servername, context) {
  if (!servername) {
    throw new ERR_TLS_REQUIRED_SERVER_NAME();
  }
  this._contexts.push([servername, context]);
};

// ---------------------------------------------------------------------------
// connect
// ---------------------------------------------------------------------------

function onConnectEnd() {
  if (!this._hadError) {
    const options = this[kConnectOptions];
    this._hadError = true;
    const error = connResetException(
      "Client network socket disconnected " +
        "before secure TLS connection was " +
        "established",
    );
    error.path = options.path;
    error.host = options.host;
    error.port = options.port;
    error.localAddress = options.localAddress;
    this.destroy(error);
  }
}

function onConnectSecure() {
  const options = this[kConnectOptions];

  let verifyError = makeVerifyError(this._handle.verifyError());

  // Verify that server's identity matches its certificate's names
  if (!verifyError && !this.isSessionReused()) {
    const hostname = options.servername ||
      options.host ||
      options.socket?._host ||
      "localhost";
    const cert = this.getPeerCertificate(true);
    verifyError = options.checkServerIdentity(hostname, cert);
  }

  if (verifyError) {
    this.authorized = false;
    this.authorizationError = verifyError.code || verifyError.message;

    if (options.rejectUnauthorized !== false) {
      this.destroy(verifyError);
      return;
    }
    debug(
      "client emit secureConnect. rejectUnauthorized: %s, " +
        "authorizationError: %s",
      options.rejectUnauthorized,
      this.authorizationError,
    );
  } else {
    this.authorized = true;
    debug("client emit secureConnect. authorized:", this.authorized);
  }

  this.secureConnecting = false;
  this.emit("secureConnect");

  this[kIsVerified] = true;
  const session = this[kPendingSession];
  this[kPendingSession] = null;
  if (session) {
    this.emit("session", session);
  }

  this.removeListener("end", onConnectEnd);
}

function normalizeConnectArgs(listArgs) {
  const args = net._normalizeArgs(listArgs);
  const options = args[0];
  const cb = args[1];

  if (listArgs[1] !== null && typeof listArgs[1] === "object") {
    ObjectAssign(options, listArgs[1]);
  } else if (listArgs[2] !== null && typeof listArgs[2] === "object") {
    ObjectAssign(options, listArgs[2]);
  }

  return cb ? [options, cb] : [options];
}

function connect(...args) {
  args = normalizeConnectArgs(args);
  let options = args[0];
  const cb = args[1];
  const allowUnauthorized = getAllowUnauthorized();

  options = {
    rejectUnauthorized: !allowUnauthorized,
    ciphers: DEFAULT_CIPHERS,
    checkServerIdentity,
    minDHSize: 1024,
    ...options,
  };

  if (!options.keepAlive) {
    options.singleUse = true;
  }

  validateFunction(options.checkServerIdentity, "options.checkServerIdentity");
  validateNumber(options.minDHSize, "options.minDHSize", 1);

  const context = options.secureContext || createSecureContext(options);

  // Default servername to host for SNI (matches Node.js behavior)
  if (!options.servername && options.host && !net.isIP(options.host)) {
    options.servername = options.host;
  }

  const tlssock = new TLSSocket(options.socket, {
    allowHalfOpen: options.allowHalfOpen,
    pipe: !!options.path,
    secureContext: context,
    isServer: false,
    requestCert: true,
    rejectUnauthorized: options.rejectUnauthorized !== false,
    session: options.session,
    ALPNProtocols: options.ALPNProtocols,
    highWaterMark: options.highWaterMark,
    servername: options.servername,
    onread: options.onread,
    signal: options.signal,
  });

  options.rejectUnauthorized = options.rejectUnauthorized !== false;

  tlssock[kConnectOptions] = options;

  if (cb) {
    tlssock.once("secureConnect", cb);
  }

  if (!options.socket) {
    if (options.timeout) {
      tlssock.setTimeout(options.timeout);
    }
    tlssock.connect(options, tlssock._start);
  }

  tlssock._releaseControl();

  if (options.session) {
    tlssock.setSession(options.session);
  }

  if (options.servername) {
    tlssock.setServername(options.servername);
  }

  if (options.socket) {
    tlssock._start();
  }

  tlssock.on("secure", onConnectSecure);
  tlssock.prependListener("end", onConnectEnd);

  return tlssock;
}

function getAllowUnauthorized() {
  return false;
}

function createServer(options, listener) {
  return new Server(options, listener);
}

// ---------------------------------------------------------------------------
// checkServerIdentity - certificate hostname verification
// ---------------------------------------------------------------------------

const jsonStringPattern =
  // deno-lint-ignore no-control-regex
  /^"(?:[^"\\\u0000-\u001f]|\\(?:["\\/bfnrt]|u[0-9a-fA-F]{4}))*"/;

function splitEscapedAltNames(altNames) {
  const result = [];
  let currentToken = "";
  let offset = 0;
  while (offset !== altNames.length) {
    const nextSep = altNames.indexOf(",", offset);
    const nextQuote = altNames.indexOf('"', offset);
    if (nextQuote !== -1 && (nextSep === -1 || nextQuote < nextSep)) {
      currentToken += altNames.substring(offset, nextQuote);
      const match = jsonStringPattern.exec(altNames.substring(nextQuote));
      if (!match) {
        const err = new Error("Invalid alt name format");
        err.code = "ERR_TLS_CERT_ALTNAME_FORMAT";
        throw err;
      }
      currentToken += JSON.parse(match[0]);
      offset = nextQuote + match[0].length;
    } else if (nextSep !== -1) {
      currentToken += altNames.substring(offset, nextSep);
      result.push(currentToken);
      currentToken = "";
      offset = nextSep + 2;
    } else {
      currentToken += altNames.substring(offset);
      offset = altNames.length;
    }
  }
  result.push(currentToken);
  return result;
}

function unfqdn(host) {
  return StringPrototypeReplace(host, /[.]$/, "");
}

function toLowerCase(c) {
  return String.fromCharCode(32 + c.charCodeAt(0));
}

function splitHost(host) {
  return unfqdn(host).replace(/[A-Z]/g, toLowerCase).split(".");
}

function check(hostParts, pattern, wildcards) {
  if (!pattern) return false;

  const patternParts = splitHost(pattern);

  if (hostParts.length !== patternParts.length) return false;
  if (patternParts.includes("")) return false;

  const isBad = (s) => /[^\u0021-\u007F]/u.test(s);
  if (patternParts.some(isBad)) return false;

  for (let i = hostParts.length - 1; i > 0; i -= 1) {
    if (hostParts[i] !== patternParts[i]) return false;
  }

  const hostSubdomain = hostParts[0];
  const patternSubdomain = patternParts[0];
  const patternSubdomainParts = patternSubdomain.split("*", 3);

  if (
    patternSubdomainParts.length === 1 ||
    patternSubdomain.includes("xn--")
  ) {
    return hostSubdomain === patternSubdomain;
  }

  if (!wildcards) return false;
  if (patternSubdomainParts.length > 2) return false;
  if (patternParts.length <= 2) return false;

  const { 0: prefix, 1: suffix } = patternSubdomainParts;
  if (prefix.length + suffix.length > hostSubdomain.length) return false;
  if (!hostSubdomain.startsWith(prefix)) return false;
  if (!hostSubdomain.endsWith(suffix)) return false;

  return true;
}

function checkServerIdentity(hostname, cert) {
  const subject = cert.subject;
  const altNames = cert.subjectaltname;
  const dnsNames = [];
  const ips = [];

  hostname = "" + hostname;

  if (altNames) {
    const splitAltNames = altNames.includes('"')
      ? splitEscapedAltNames(altNames)
      : altNames.split(", ");
    splitAltNames.forEach((name) => {
      if (name.startsWith("DNS:")) {
        dnsNames.push(name.slice(4));
      } else if (name.startsWith("IP Address:")) {
        ips.push(canonicalizeIP(name.slice(11)));
      }
    });
  }

  let valid = false;
  let reason = "Unknown reason";

  hostname = unfqdn(hostname);

  if (net.isIP(hostname)) {
    valid = ips.includes(canonicalizeIP(hostname));
    if (!valid) {
      reason = `IP: ${hostname} is not in the cert's list: ` + ips.join(", ");
    }
  } else if (dnsNames.length > 0 || subject?.CN) {
    const hostParts = splitHost(hostname);
    const wildcard = (pattern) => check(hostParts, pattern, true);

    if (dnsNames.length > 0) {
      valid = dnsNames.some(wildcard);
      if (!valid) {
        reason =
          `Host: ${hostname}. is not in the cert's altnames: ${altNames}`;
      }
    } else {
      const cn = subject.CN;

      if (ArrayIsArray(cn)) {
        valid = cn.some(wildcard);
      } else if (cn) {
        valid = wildcard(cn);
      }

      if (!valid) {
        reason = `Host: ${hostname}. is not cert's CN: ${cn}`;
      }
    }
  } else {
    reason = "Cert does not contain a DNS name";
  }

  if (!valid) {
    return new ERR_TLS_CERT_ALTNAME_INVALID(reason, hostname, cert);
  }
}

// Order matters. Mirrors ALL_CIPHER_SUITES from rustls/src/suites.rs but
// using openssl cipher names instead.
const DEFAULT_CIPHERS = [
  // TLSv1.3 suites
  "AES256-GCM-SHA384",
  "AES128-GCM-SHA256",
  "TLS_CHACHA20_POLY1305_SHA256",
  // TLSv1.2 suites
  "ECDHE-ECDSA-AES256-GCM-SHA384",
  "ECDHE-ECDSA-AES128-GCM-SHA256",
  "ECDHE-ECDSA-CHACHA20-POLY1305",
  "ECDHE-RSA-AES256-GCM-SHA384",
  "ECDHE-RSA-AES128-GCM-SHA256",
  "ECDHE-RSA-CHACHA20-POLY1305",
].join(":");

export {
  checkServerIdentity,
  connect,
  createServer,
  DEFAULT_CIPHERS,
  Server,
  TLSSocket,
  unfqdn,
};

export default {
  TLSSocket,
  connect,
  createServer,
  checkServerIdentity,
  DEFAULT_CIPHERS,
  Server,
  unfqdn,
};
