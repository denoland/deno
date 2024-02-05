// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
const { internalRidSymbol } = core;
import {
  op_net_accept_tls,
  op_net_connect_tls,
  op_net_listen_tls,
  op_tls_handshake,
  op_tls_start,
} from "ext:core/ops";
const {
  Number,
  ObjectDefineProperty,
  TypeError,
} = primordials;

import { Conn, Listener } from "ext:deno_net/01_net.js";

function opStartTls(args) {
  return op_tls_start(args);
}

function opTlsHandshake(rid) {
  return op_tls_handshake(rid);
}

class TlsConn extends Conn {
  #rid = 0;

  constructor(rid, remoteAddr, localAddr) {
    super(rid, remoteAddr, localAddr);
    ObjectDefineProperty(this, internalRidSymbol, {
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
  }

  get rid() {
    internals.warnOnDeprecatedApi(
      "Deno.TlsConn.rid",
      new Error().stack,
      "Use `Deno.TlsConn` instance methods instead.",
    );
    return this.#rid;
  }

  handshake() {
    return opTlsHandshake(this.#rid);
  }
}

async function connectTls({
  port,
  hostname = "127.0.0.1",
  transport = "tcp",
  certFile = undefined,
  caCerts = [],
  certChain = undefined,
  privateKey = undefined,
  cert = undefined,
  key = undefined,
  alpnProtocols = undefined,
}) {
  if (certFile !== undefined) {
    internals.warnOnDeprecatedApi(
      "Deno.ConnectTlsOptions.certFile",
      new Error().stack,
      "Pass the cert file contents to the `Deno.ConnectTlsOptions.certChain` option instead.",
    );
  }
  if (transport !== "tcp") {
    throw new TypeError(`Unsupported transport: '${transport}'`);
  }
  if (certChain !== undefined && cert !== undefined) {
    throw new TypeError(
      "Cannot specify both `certChain` and `cert`",
    );
  }
  if (privateKey !== undefined && key !== undefined) {
    throw new TypeError(
      "Cannot specify both `privateKey` and `key`",
    );
  }
  certChain ??= cert;
  privateKey ??= key;
  const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_connect_tls(
    { hostname, port },
    { certFile, caCerts, certChain, privateKey, alpnProtocols },
  );
  localAddr.transport = "tcp";
  remoteAddr.transport = "tcp";
  return new TlsConn(rid, remoteAddr, localAddr);
}

class TlsListener extends Listener {
  #rid = 0;

  constructor(rid, addr) {
    super(rid, addr);
    ObjectDefineProperty(this, internalRidSymbol, {
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
  }

  get rid() {
    internals.warnOnDeprecatedApi(
      "Deno.TlsListener.rid",
      new Error().stack,
      "Use `Deno.TlsListener` instance methods instead.",
    );
    return this.#rid;
  }

  async accept() {
    const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_accept_tls(
      this.#rid,
    );
    localAddr.transport = "tcp";
    remoteAddr.transport = "tcp";
    return new TlsConn(rid, remoteAddr, localAddr);
  }
}

function listenTls({
  port,
  cert,
  certFile,
  key,
  keyFile,
  hostname = "0.0.0.0",
  transport = "tcp",
  alpnProtocols = undefined,
  reusePort = false,
}) {
  if (transport !== "tcp") {
    throw new TypeError(`Unsupported transport: '${transport}'`);
  }
  if (keyFile !== undefined) {
    internals.warnOnDeprecatedApi(
      "Deno.ListenTlsOptions.keyFile",
      new Error().stack,
      "Pass the key file contents to the `Deno.ListenTlsOptions.key` option instead.",
    );
  }
  if (certFile !== undefined) {
    internals.warnOnDeprecatedApi(
      "Deno.ListenTlsOptions.certFile",
      new Error().stack,
      "Pass the cert file contents to the `Deno.ListenTlsOptions.cert` option instead.",
    );
  }
  const { 0: rid, 1: localAddr } = op_net_listen_tls(
    { hostname, port: Number(port) },
    { cert, certFile, key, keyFile, alpnProtocols, reusePort },
  );
  return new TlsListener(rid, localAddr);
}

async function startTls(
  conn,
  {
    hostname = "127.0.0.1",
    certFile = undefined,
    caCerts = [],
    alpnProtocols = undefined,
  } = {},
) {
  const { 0: rid, 1: localAddr, 2: remoteAddr } = await opStartTls({
    rid: conn[internalRidSymbol],
    hostname,
    certFile,
    caCerts,
    alpnProtocols,
  });
  return new TlsConn(rid, remoteAddr, localAddr);
}

export { connectTls, listenTls, startTls, TlsConn, TlsListener };
