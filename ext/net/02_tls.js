// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
const { internalRidSymbol } = core;
import {
  op_net_accept_tls,
  op_net_connect_tls,
  op_net_listen_tls,
  op_tls_handshake,
  op_tls_key_null,
  op_tls_key_static,
  op_tls_key_static_from_file,
  op_tls_start,
} from "ext:core/ops";
const {
  Number,
  ObjectDefineProperty,
  ReflectHas,
  TypeError,
} = primordials;

import { Conn, Listener } from "ext:deno_net/01_net.js";

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
    return op_tls_handshake(this.#rid);
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
      "Pass the cert file contents to the `Deno.ConnectTlsOptions.cert` option instead.",
    );
  }
  if (certChain !== undefined) {
    internals.warnOnDeprecatedApi(
      "Deno.ConnectTlsOptions.certChain",
      new Error().stack,
      "Use the `Deno.ConnectTlsOptions.cert` option instead.",
    );
  }
  if (privateKey !== undefined) {
    internals.warnOnDeprecatedApi(
      "Deno.ConnectTlsOptions.privateKey",
      new Error().stack,
      "Use the `Deno.ConnectTlsOptions.key` option instead.",
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
  cert ??= certChain;
  key ??= privateKey;
  const keyPair = loadTlsKeyPair(cert, undefined, key, undefined);
  const { 0: rid, 1: localAddr, 2: remoteAddr } = await op_net_connect_tls(
    { hostname, port },
    { certFile, caCerts, cert, key, alpnProtocols },
    keyPair,
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

function hasTlsKeyPairOptions(options) {
  return (ReflectHas(options, "cert") || ReflectHas(options, "key") ||
    ReflectHas(options, "certFile") ||
    ReflectHas(options, "keyFile"));
}

function loadTlsKeyPair(
  cert,
  certFile,
  key,
  keyFile,
) {
  if ((certFile !== undefined) ^ (keyFile !== undefined)) {
    throw new TypeError(
      "If certFile is specified, keyFile must also be specified",
    );
  }
  if ((cert !== undefined) ^ (key !== undefined)) {
    throw new TypeError("If cert is specified, key must also be specified");
  }

  if (certFile !== undefined) {
    return op_tls_key_static_from_file("Deno.listenTls", certFile, keyFile);
  } else if (cert !== undefined) {
    return op_tls_key_static(cert, key);
  } else {
    return op_tls_key_null();
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

  const keyPair = loadTlsKeyPair(cert, certFile, key, keyFile);
  const { 0: rid, 1: localAddr } = op_net_listen_tls(
    { hostname, port: Number(port) },
    { alpnProtocols, reusePort },
    keyPair,
  );
  return new TlsListener(rid, localAddr);
}

// deno-lint-ignore require-await
async function startTls(
  conn,
  {
    hostname = "127.0.0.1",
    caCerts = [],
    alpnProtocols = undefined,
  } = {},
) {
  const { 0: rid, 1: localAddr, 2: remoteAddr } = op_tls_start({
    rid: conn[internalRidSymbol],
    hostname,
    caCerts,
    alpnProtocols,
  });
  return new TlsConn(rid, remoteAddr, localAddr);
}

export {
  connectTls,
  hasTlsKeyPairOptions,
  listenTls,
  loadTlsKeyPair,
  startTls,
  TlsConn,
  TlsListener,
};
